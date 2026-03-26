#Requires -Version 7.0
<#
.SYNOPSIS
    Non-interactive JSON wrapper for assumeIdentity (used by the FNBA Utils app).
#>
param(
    [Parameter(Position=0, Mandatory)] [string]$User,
    [Parameter(Position=1, Mandatory)] [string]$Connection
)

# Save params before dot-sourcing — the main script's param() block
# runs in this scope and would overwrite $User/$Connection with empty defaults.
$_user = $User
$_conn = $Connection

# Dot-source the main script to get all helper functions
. "$PSScriptRoot\assumeIdentity.ps1" -LoadOnly

$data     = Get-StoredData
$toAssume = Resolve-User $data $_user
$server   = Resolve-Conn $data $_conn

if (-not $toAssume) {
    @{ server = $Connection; login = "FNBA\$IMPOSTER"; before = $null; after = $null; passwordChanged = $false; alreadyAssuming = $false; message = "Unknown user: '$User'" } | ConvertTo-Json -Depth 3
    exit 1
}

if (-not $server) {
    @{ server = $Connection; login = "FNBA\$IMPOSTER"; before = $null; after = $null; passwordChanged = $false; alreadyAssuming = $false; message = "Unknown connection: '$Connection'" } | ConvertTo-Json -Depth 3
    exit 1
}

# Pre-flight check
$current = Get-CurrentIdentity -server $server -toAssume $toAssume

if ($null -ne $current -and [int]$current["already_assuming"] -eq 1) {
    $actingAsLogin = if ([string]::IsNullOrWhiteSpace($current["acting_as_login"])) { "FNBA\$IMPOSTER" } else { $current["acting_as_login"] }
    $actingAsName  = if ([string]::IsNullOrWhiteSpace($current["acting_as_login"])) { "self" }           else { $current["acting_as_name"]  }

    @{
        server          = $server
        login           = "FNBA\$IMPOSTER"
        before          = $null
        after           = @{
            acting_as_login = $actingAsLogin
            acting_as_name  = $actingAsName
            password        = "$($current["password"])"
            changed_at      = "{0:HH:mm:ss MM-dd-yyyy}" -f $current["changed_at"]
            on_host         = $server
        }
        passwordChanged = $false
        alreadyAssuming = $true
        message         = "Already acting as this identity - no change needed."
    } | ConvertTo-Json -Depth 3
    exit 0
}

# Execute the identity switch using the same SQL logic
$lines = @(
    "CREATE TABLE #states ("
    "    phase          VARCHAR(7)   NOT NULL,"
    "    imposter_login VARCHAR(35)  NOT NULL,"
    "    acting_as_name  VARCHAR(71)  NOT NULL,"
    "    acting_as_login VARCHAR(35)  NULL,"
    "    password       VARCHAR(MAX) NOT NULL,"
    "    changed_at     DATETIME     NOT NULL,"
    "    on_host        VARCHAR(35)  NOT NULL"
    ");"
    "DECLARE @Imposter VARCHAR(35) = '$IMPOSTER';"
    "DECLARE @ToAssume VARCHAR(35) = '$toAssume';"
    "DECLARE @ImposterLogin VARCHAR(35) = 'FNBA\' + '$IMPOSTER';"
    "DECLARE @ToAssumeLogin VARCHAR(35) = 'FNBA\' + '$toAssume';"
    "DECLARE @CurrentAssocId INT;"
    "DECLARE @TargetAssocId  INT;"
    "SELECT @CurrentAssocId = assoc_id FROM logincheck.fnba_reporting.associate_login WHERE domain_username = @ImposterLogin;"
    "SELECT @TargetAssocId  = assoc_id FROM logincheck.fnba_reporting.associate_login WHERE domain_username = @ToAssumeLogin;"
    "INSERT INTO #states"
    "SELECT 'before',"
    "    al.domain_username,"
    "    per.first_name + ' ' + per.last_name,"
    "    (SELECT ISNULL(MIN(domain_username), @ImposterLogin) FROM logincheck.fnba_reporting.associate_login WHERE assoc_id = al.assoc_id AND domain_username <> @ImposterLogin),"
    "    al.password,"
    "    al.date_modified,"
    "    @@SERVERNAME"
    "FROM logincheck.fnba_reporting.associate_login al"
    "JOIN perdb.fnba.associate per ON per.assoc_id = al.assoc_id"
    "WHERE al.domain_username = @ImposterLogin;"
    "EXEC logincheck.fnba.assumeIdentity"
    "    @windowsLoginOfImpostor = @Imposter,"
    "    @windowsLoginToAssume   = @ToAssume;"
    "INSERT INTO #states"
    "SELECT 'after',"
    "    al.domain_username,"
    "    per.first_name + ' ' + per.last_name,"
    "    @ToAssumeLogin,"
    "    al.password,"
    "    al.date_modified,"
    "    @@SERVERNAME"
    "FROM logincheck.fnba_reporting.associate_login al"
    "JOIN perdb.fnba.associate per ON per.assoc_id = al.assoc_id"
    "WHERE al.domain_username = @ImposterLogin;"
    "SELECT * FROM #states;"
    "DROP TABLE #states;"
)
$sql = $lines -join " "

$connStr = "Server=$server;Database=logincheck;Integrated Security=SSPI;TrustServerCertificate=True;"
$conn    = New-Object System.Data.SqlClient.SqlConnection $connStr
$cmd     = New-Object System.Data.SqlClient.SqlCommand($sql, $conn)
$adapter = New-Object System.Data.SqlClient.SqlDataAdapter $cmd
$table   = New-Object System.Data.DataTable

try {
    $conn.Open()
    [void]$adapter.Fill($table)

    if ($table.Rows.Count -lt 2) {
        @{ server = $server; login = "FNBA\$IMPOSTER"; before = $null; after = $null; passwordChanged = $false; alreadyAssuming = $false; message = "Unexpected result - expected before and after rows." } | ConvertTo-Json -Depth 3
        exit 1
    }

    $before  = $table.Rows[0]
    $after   = $table.Rows[1]
    $changed = $before["password"] -ne $after["password"]

    $beforeActingAsLogin = if ([string]::IsNullOrWhiteSpace($before["acting_as_login"])) { $before["imposter_login"] } else { $before["acting_as_login"] }
    $beforeActingAsName  = if ([string]::IsNullOrWhiteSpace($before["acting_as_login"])) { "self" }                    else { $before["acting_as_name"]  }

    @{
        server          = $server
        login           = "FNBA\$IMPOSTER"
        before          = @{
            acting_as_login = "$beforeActingAsLogin"
            acting_as_name  = "$beforeActingAsName"
            password        = "$($before["password"])"
            changed_at      = "{0:HH:mm:ss MM-dd-yyyy}" -f $before["changed_at"]
            on_host         = "$($before["on_host"])"
        }
        after           = @{
            acting_as_login = "$($after["acting_as_login"])"
            acting_as_name  = "$($after["acting_as_name"])"
            password        = "$($after["password"])"
            changed_at      = "{0:HH:mm:ss MM-dd-yyyy}" -f $after["changed_at"]
            on_host         = "$($after["on_host"])"
        }
        passwordChanged = $changed
        alreadyAssuming = $false
        message         = if ($changed) { "Identity switched successfully." } else { "WARNING: Password did not change - the identity switch may have failed." }
    } | ConvertTo-Json -Depth 3

} catch {
    @{ server = $server; login = "FNBA\$IMPOSTER"; before = $null; after = $null; passwordChanged = $false; alreadyAssuming = $false; message = "ERROR: $($_.Exception.Message)" } | ConvertTo-Json -Depth 3
    exit 1
} finally {
    $conn.Close()
}
