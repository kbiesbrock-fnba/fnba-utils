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
    let mut config = Config::new();
    config.host(server);
    config.port(1433);
    config.database("logincheck");
    config.authentication(AuthMethod::Integrated);
    config.trust_cert();

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

pub struct PreflightRow {
    pub already_assuming: bool,
    pub password: String,
    pub changed_at: String,
    pub acting_as_login: Option<String>,
    pub acting_as_name: Option<String>,
}

/// Pre-flight check: is the imposter already assuming the target identity?
pub async fn check_current_identity(
    client: &mut SqlClient,
    imposter: &str,
    to_assume: &str,
) -> Result<Option<PreflightRow>, String> {
    let sql = format!(
        "\
DECLARE @ImposterLogin VARCHAR(35) = 'FNBA\\{imposter}';
DECLARE @ToAssumeLogin VARCHAR(35) = 'FNBA\\{to_assume}';
DECLARE @CurrentAssocId INT;
DECLARE @TargetAssocId  INT;
SELECT @CurrentAssocId = assoc_id FROM logincheck.fnba_reporting.associate_login WHERE domain_username = @ImposterLogin;
SELECT @TargetAssocId  = assoc_id FROM logincheck.fnba_reporting.associate_login WHERE domain_username = @ToAssumeLogin;
SELECT
    CASE WHEN @CurrentAssocId = @TargetAssocId THEN 1 ELSE 0 END AS already_assuming,
    al.password,
    al.date_modified AS changed_at,
    (SELECT MIN(domain_username) FROM logincheck.fnba_reporting.associate_login WHERE assoc_id = al.assoc_id AND domain_username <> @ImposterLogin) AS acting_as_login,
    per.first_name + ' ' + per.last_name AS acting_as_name
FROM logincheck.fnba_reporting.associate_login al
JOIN perdb.fnba.associate per ON per.assoc_id = al.assoc_id
WHERE al.domain_username = @ImposterLogin;"
    );

    let results = client
        .simple_query(&sql)
        .await
        .map_err(|e| format!("Pre-flight query failed: {e}"))?
        .into_results()
        .await
        .map_err(|e| format!("Pre-flight result read failed: {e}"))?;

    // Find the last non-empty result set (the SELECT output)
    let rows = results.into_iter().rev().find(|rs| !rs.is_empty());
    let Some(rows) = rows else {
        return Ok(None);
    };
    let row = &rows[0];

    Ok(Some(PreflightRow {
        already_assuming: row.try_get::<i32, _>("already_assuming")
            .map_err(|e| format!("Column read error: {e}"))?
            .unwrap_or(0)
            == 1,
        password: row.try_get::<&str, _>("password")
            .map_err(|e| format!("Column read error: {e}"))?
            .unwrap_or("")
            .to_string(),
        changed_at: format_datetime(
            row.try_get::<NaiveDateTime, _>("changed_at")
                .map_err(|e| format!("Column read error: {e}"))?,
        ),
        acting_as_login: row.try_get::<&str, _>("acting_as_login")
            .map_err(|e| format!("Column read error: {e}"))?
            .map(|s| s.to_string()),
        acting_as_name: row.try_get::<&str, _>("acting_as_name")
            .map_err(|e| format!("Column read error: {e}"))?
            .map(|s| s.to_string()),
    }))
}

/// Execute the identity switch and return (before, after) states.
pub async fn run_identity_switch(
    client: &mut SqlClient,
    imposter: &str,
    to_assume: &str,
) -> Result<(IdentityState, IdentityState), String> {
    let sql = format!(
        "\
CREATE TABLE #states (
    phase          VARCHAR(7)   NOT NULL,
    imposter_login VARCHAR(35)  NOT NULL,
    acting_as_name VARCHAR(71)  NOT NULL,
    acting_as_login VARCHAR(35) NULL,
    password       VARCHAR(MAX) NOT NULL,
    changed_at     DATETIME     NOT NULL,
    on_host        VARCHAR(35)  NOT NULL
);
DECLARE @Imposter VARCHAR(35) = '{imposter}';
DECLARE @ToAssume VARCHAR(35) = '{to_assume}';
DECLARE @ImposterLogin VARCHAR(35) = 'FNBA\\' + '{imposter}';
DECLARE @ToAssumeLogin VARCHAR(35) = 'FNBA\\' + '{to_assume}';
DECLARE @CurrentAssocId INT;
DECLARE @TargetAssocId  INT;
SELECT @CurrentAssocId = assoc_id FROM logincheck.fnba_reporting.associate_login WHERE domain_username = @ImposterLogin;
SELECT @TargetAssocId  = assoc_id FROM logincheck.fnba_reporting.associate_login WHERE domain_username = @ToAssumeLogin;
INSERT INTO #states
SELECT 'before',
    al.domain_username,
    per.first_name + ' ' + per.last_name,
    (SELECT ISNULL(MIN(domain_username), @ImposterLogin) FROM logincheck.fnba_reporting.associate_login WHERE assoc_id = al.assoc_id AND domain_username <> @ImposterLogin),
    al.password,
    al.date_modified,
    @@SERVERNAME
FROM logincheck.fnba_reporting.associate_login al
JOIN perdb.fnba.associate per ON per.assoc_id = al.assoc_id
WHERE al.domain_username = @ImposterLogin;
EXEC logincheck.fnba.assumeIdentity
    @windowsLoginOfImpostor = @Imposter,
    @windowsLoginToAssume   = @ToAssume;
INSERT INTO #states
SELECT 'after',
    al.domain_username,
    per.first_name + ' ' + per.last_name,
    @ToAssumeLogin,
    al.password,
    al.date_modified,
    @@SERVERNAME
FROM logincheck.fnba_reporting.associate_login al
JOIN perdb.fnba.associate per ON per.assoc_id = al.assoc_id
WHERE al.domain_username = @ImposterLogin;
SELECT * FROM #states;
DROP TABLE #states;"
    );

    let results = client
        .simple_query(&sql)
        .await
        .map_err(|e| format!("Identity switch query failed: {e}"))?
        .into_results()
        .await
        .map_err(|e| format!("Identity switch result read failed: {e}"))?;

    // Find the result set with our #states rows (last non-empty set)
    let rows: Vec<Row> = results
        .into_iter()
        .rev()
        .find(|rs| !rs.is_empty())
        .ok_or("Unexpected result - no rows returned from identity switch")?;

    if rows.len() < 2 {
        return Err("Unexpected result - expected before and after rows".into());
    }

    let imposter_login = format!("FNBA\\{imposter}");
    let before = parse_state_row(&rows[0], &imposter_login)?;
    let after = parse_state_row(&rows[1], &imposter_login)?;

    Ok((before, after))
}

fn parse_state_row(row: &Row, imposter_login: &str) -> Result<IdentityState, String> {
    let acting_as_login_raw = row
        .try_get::<&str, _>("acting_as_login")
        .map_err(|e| format!("Column read error: {e}"))?;

    let (acting_as_login, acting_as_name) =
        if acting_as_login_raw.is_none() || acting_as_login_raw.unwrap_or("").trim().is_empty() {
            (
                imposter_login.to_string(),
                "self".to_string(),
            )
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
