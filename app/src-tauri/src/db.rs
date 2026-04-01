use crate::models::identity::IdentityState;
use chrono::NaiveDateTime;
use std::time::Duration;
use tiberius::{AuthMethod, Client, Config, Row};
use tokio::net::TcpStream;
use tokio::time::timeout;
use tokio_util::compat::{Compat, TokioAsyncWriteCompatExt};

pub type SqlClient = Client<Compat<TcpStream>>;

const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);

pub async fn connect(server: &str) -> Result<SqlClient, String> {
    connect_to(server, "logincheck").await
}

pub async fn connect_to(server: &str, database: &str) -> Result<SqlClient, String> {
    let mut config = Config::new();
    config.host(server);
    config.port(1433);
    config.database(database);
    config.authentication(AuthMethod::Integrated);
    if cfg!(debug_assertions) {
        config.trust_cert();
    }

    let tcp = timeout(CONNECT_TIMEOUT, TcpStream::connect((server, 1433u16)))
        .await
        .map_err(|_| format!("Connection to {server} timed out after {CONNECT_TIMEOUT:?}"))?
        .map_err(|e| format!("TCP connection to {server} failed: {e}"))?;

    tcp.set_nodelay(true)
        .map_err(|e| format!("set_nodelay failed: {e}"))?;

    let client: SqlClient = Client::connect(config, tcp.compat_write())
        .await
        .map_err(|e| format!("SQL authentication to {server} failed: {e}"))?;

    Ok(client)
}

pub enum SwitchOutcome {
    AlreadyAssuming {
        acting_as_login: String,
        acting_as_name: String,
        password: String,
        changed_at: String,
        on_host: String,
    },
    Switched {
        before: IdentityState,
        after: IdentityState,
    },
    ImposterNotFound,
}

pub async fn assume_identity(
    client: &mut SqlClient,
    imposter: &str,
    to_assume: &str,
) -> Result<SwitchOutcome, String> {
    let sql = "\
DECLARE @Imposter      VARCHAR(35) = @P1;
DECLARE @ToAssume      VARCHAR(35) = @P2;
DECLARE @ImposterLogin VARCHAR(35) = 'FNBA\\' + @P1;
DECLARE @ToAssumeLogin VARCHAR(35) = 'FNBA\\' + @P2;

DECLARE @CurrentAssocId INT;
DECLARE @TargetAssocId  INT;
SELECT @CurrentAssocId = assoc_id FROM logincheck.fnba_reporting.associate_login WHERE domain_username = @ImposterLogin;
SELECT @TargetAssocId  = assoc_id FROM logincheck.fnba_reporting.associate_login WHERE domain_username = @ToAssumeLogin;

DECLARE @BeforeActingAsLogin VARCHAR(35);
DECLARE @BeforeActingAsName  VARCHAR(71);
DECLARE @BeforePassword      VARCHAR(MAX);
DECLARE @BeforeChangedAt     DATETIME;
DECLARE @OnHost              VARCHAR(35) = @@SERVERNAME;

SELECT
    @BeforeActingAsLogin = (
        SELECT ISNULL(MIN(domain_username), @ImposterLogin)
          FROM logincheck.fnba_reporting.associate_login
         WHERE assoc_id = al.assoc_id
           AND domain_username <> @ImposterLogin
    ),
    @BeforeActingAsName = per.first_name + ' ' + per.last_name,
    @BeforePassword     = al.password,
    @BeforeChangedAt    = al.date_modified
FROM logincheck.fnba_reporting.associate_login al
JOIN perdb.fnba.associate per ON per.assoc_id = al.assoc_id
WHERE al.domain_username = @ImposterLogin;

IF @CurrentAssocId IS NULL
BEGIN
    SELECT 'not_found' AS phase;
END
ELSE IF @CurrentAssocId = @TargetAssocId
BEGIN
    SELECT
        'current'            AS phase,
        @ImposterLogin       AS imposter_login,
        @BeforeActingAsLogin AS acting_as_login,
        @BeforeActingAsName  AS acting_as_name,
        @BeforePassword      AS password,
        @BeforeChangedAt     AS changed_at,
        @OnHost              AS on_host;
END
ELSE
BEGIN
    SELECT
        'before'             AS phase,
        @ImposterLogin       AS imposter_login,
        @BeforeActingAsLogin AS acting_as_login,
        @BeforeActingAsName  AS acting_as_name,
        @BeforePassword      AS password,
        @BeforeChangedAt     AS changed_at,
        @OnHost              AS on_host;

    EXEC logincheck.fnba.assumeIdentity
        @windowsLoginOfImpostor = @Imposter,
        @windowsLoginToAssume   = @ToAssume;

    SELECT
        'after'          AS phase,
        @ImposterLogin   AS imposter_login,
        @ToAssumeLogin   AS acting_as_login,
        per.first_name + ' ' + per.last_name AS acting_as_name,
        al.password,
        al.date_modified AS changed_at,
        @OnHost          AS on_host
    FROM logincheck.fnba_reporting.associate_login al
    JOIN perdb.fnba.associate per ON per.assoc_id = al.assoc_id
    WHERE al.domain_username = @ImposterLogin;
END";

    let results = client
        .query(sql, &[&imposter, &to_assume])
        .await
        .map_err(|e| format!("Identity query failed: {e}"))?
        .into_results()
        .await
        .map_err(|e| format!("Identity result read failed: {e}"))?;

    let sets: Vec<Vec<Row>> = results.into_iter().filter(|rs| !rs.is_empty()).collect();

    if sets.is_empty() {
        return Ok(SwitchOutcome::ImposterNotFound);
    }

    // Use the phase column to find rows (immune to extra result sets from the stored proc)
    let mut current_row: Option<&Row> = None;
    let mut before_row: Option<&Row> = None;
    let mut after_row: Option<&Row> = None;
    let mut not_found = false;

    for set in &sets {
        if let Some(row) = set.first() {
            match row.try_get::<&str, _>("phase") {
                Ok(Some("not_found")) => not_found = true,
                Ok(Some("current")) => current_row = Some(row),
                Ok(Some("before")) => before_row = Some(row),
                Ok(Some("after")) => after_row = Some(row),
                _ => {} // skip result sets from the stored proc
            }
        }
    }

    if not_found {
        return Ok(SwitchOutcome::ImposterNotFound);
    }

    if let Some(row) = current_row {
        return Ok(SwitchOutcome::AlreadyAssuming {
            acting_as_login: parse_login(row, imposter),
            acting_as_name: row
                .try_get::<&str, _>("acting_as_name")
                .map_err(|e| format!("Column read error: {e}"))?
                .unwrap_or("unknown")
                .to_string(),
            password: row
                .try_get::<&str, _>("password")
                .map_err(|e| format!("Column read error: {e}"))?
                .unwrap_or("")
                .to_string(),
            changed_at: format_datetime(
                row.try_get::<NaiveDateTime, _>("changed_at")
                    .map_err(|e| format!("Column read error: {e}"))?,
            ),
            on_host: row
                .try_get::<&str, _>("on_host")
                .map_err(|e| format!("Column read error: {e}"))?
                .unwrap_or("")
                .to_string(),
        });
    }

    let before_row = before_row.ok_or("Missing 'before' row in identity switch result")?;
    let after_row = after_row.ok_or("Missing 'after' row - stored proc may have failed")?;

    let imposter_login = format!("FNBA\\{imposter}");
    let before = parse_state_row(before_row, &imposter_login)?;
    let after = parse_state_row(after_row, &imposter_login)?;

    Ok(SwitchOutcome::Switched { before, after })
}

fn parse_login(row: &Row, imposter: &str) -> String {
    let login = row
        .try_get::<&str, _>("acting_as_login")
        .ok()
        .flatten()
        .unwrap_or("")
        .trim();
    if login.is_empty() {
        format!("FNBA\\{imposter}")
    } else {
        login.to_string()
    }
}

fn parse_state_row(row: &Row, imposter_login: &str) -> Result<IdentityState, String> {
    let acting_as_login_raw = row
        .try_get::<&str, _>("acting_as_login")
        .map_err(|e| format!("Column read error: {e}"))?;

    let (acting_as_login, acting_as_name) =
        if acting_as_login_raw.is_none() || acting_as_login_raw.unwrap_or("").trim().is_empty() {
            (imposter_login.to_string(), "self".to_string())
        } else {
            (
                acting_as_login_raw.unwrap().to_string(),
                row.try_get::<&str, _>("acting_as_name")
                    .map_err(|e| format!("Column read error: {e}"))?
                    .unwrap_or("unknown")
                    .to_string(),
            )
        };

    Ok(IdentityState {
        acting_as_login,
        acting_as_name,
        password: row
            .try_get::<&str, _>("password")
            .map_err(|e| format!("Column read error: {e}"))?
            .unwrap_or("")
            .to_string(),
        changed_at: format_datetime(
            row.try_get::<NaiveDateTime, _>("changed_at")
                .map_err(|e| format!("Column read error: {e}"))?,
        ),
        on_host: row
            .try_get::<&str, _>("on_host")
            .map_err(|e| format!("Column read error: {e}"))?
            .unwrap_or("")
            .to_string(),
    })
}

fn format_datetime(dt: Option<NaiveDateTime>) -> String {
    match dt {
        Some(dt) => dt.format("%H:%M:%S %m-%d-%Y").to_string(),
        None => String::new(),
    }
}
