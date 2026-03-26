#Requires -Version 7.0
<#
.SYNOPSIS
    Assume a SQL identity against a target server - no SSMS required.

.DESCRIPTION
    Uses your Windows session token (SSPI) - no password required.
    All interaction happens at the command line via tab completion.
    Custom users and connections are saved to ~\.assumeIdentity.json.

.PARAMETER User
    Username or label keyword. Tab-completes from known users and labels.

.PARAMETER Connection
    Server hostname. Tab-completes from known connections.

.EXAMPLE
    assumeIdentity cpatterson dsqlkbiesbrock.fnba-dev.network
    assumeIdentity cpat<TAB>
    assumeIdentity batch<TAB>
    assumeIdentity cpatterson kbi<TAB>

.NOTES
    First-time setup - add to your PowerShell profile ($PROFILE):

        function assumeIdentity { pwsh -NoProfile -File "C:\Users\kbiesbrock\scripts\assumeIdentity.ps1" @args }
        Register-IdentityCompleter

    Then reload: . $PROFILE
#>

[CmdletBinding()]
param(
    [Parameter(Position=0)] [string]$User,
    [Parameter(Position=1)] [string]$Connection,
    [switch]$LoadOnly
)

$DEFAULTS_FILE = Join-Path $PSScriptRoot "identity-defaults.json"
$defaults      = Get-Content $DEFAULTS_FILE -Raw | ConvertFrom-Json

$IMPOSTER  = $defaults.imposter
$DATA_FILE = Join-Path $HOME ".assumeIdentity.json"

$DEFAULT_CONNECTIONS = @($defaults.defaultConnections)

$DEFAULT_USERS = @($defaults.defaultUsers | ForEach-Object {
    [pscustomobject]@{ Label = $_.label; Username = $_.username }
})

# -- Persistence ---------------------------------------------------------------

function Get-StoredData {
    if (Test-Path $DATA_FILE) {
        try {
            $raw = Get-Content $DATA_FILE -Raw | ConvertFrom-Json
            return @{
                CustomUsers       = @($raw.CustomUsers | ForEach-Object { [pscustomobject]$_ })
                CustomConnections = @($raw.CustomConnections)
            }
        } catch {}
    }
    return @{ CustomUsers = @(); CustomConnections = @() }
}

function Save-StoredData($data) {
    [pscustomobject]@{
        CustomUsers       = $data.CustomUsers
        CustomConnections = $data.CustomConnections
    } | ConvertTo-Json -Depth 5 | Set-Content $DATA_FILE -Encoding UTF8
}

function Get-AllUsers($data) { @($DEFAULT_USERS) + @($data.CustomUsers) }
function Get-AllConns($data) { @($DEFAULT_CONNECTIONS) + @($data.CustomConnections) }

# -- User map: one entry per username, all labels combined --------------------

function Build-UserMap {
    param([array]$Users)
    $map = [ordered]@{}
    foreach ($u in $Users) {
        if (-not $map.Contains($u.Username)) {
            $map[$u.Username] = [System.Collections.Generic.List[string]]::new()
        }
        if ($u.Label -and ($u.Label -notin $map[$u.Username])) {
            [void]$map[$u.Username].Add($u.Label)
        }
    }
    $result = @()
    foreach ($key in $map.Keys) {
        $result += [pscustomobject]@{
            Username = $key
            Labels   = ($map[$key] -join " | ")
        }
    }
    return $result
}

# -- Resolve exact or unambiguous prefix match --------------------------------

function Resolve-User($data, [string]$partial) {
    if (-not $partial) { return $null }
    $map   = Build-UserMap (Get-AllUsers $data)
    $exact = $map | Where-Object { $_.Username -ieq $partial } | Select-Object -First 1
    if ($exact) { return $exact.Username }
    $prefix = @($map | Where-Object { $_.Username -ilike "$partial*" })
    if ($prefix.Count -eq 1) { return $prefix[0].Username }
    return $null
}

function Resolve-Conn($data, [string]$partial) {
    if (-not $partial) { return $null }
    $all   = Get-AllConns $data
    $exact = $all | Where-Object { $_ -ieq $partial } | Select-Object -First 1
    if ($exact) { return $exact }
    $prefix = @($all | Where-Object { $_ -ilike "$partial*" })
    if ($prefix.Count -eq 1) { return $prefix[0] }
    return $null
}

# -- Tab completion ------------------------------------------------------------
#
#  For $User, the completer runs two passes and merges results:
#
#  Pass 1 - username prefix match
#    "cpat" -> cpatterson   [Application Rights | Rosebud]
#    "ww"   -> wwessels     [Underwriting]
#
#  Pass 2 - label keyword match (any user whose label contains the typed text,
#           that was NOT already returned in pass 1)
#    "batch"  -> clacroix, mkeller, aguggemmos, pbenson, ahanson
#    "underw" -> ajenks [Underwriter], sfortino [Credit Underwriting],
#                wwessels [Underwriting]
#    "cpat"   -> cpatterson appears only once (pass 1 wins, deduped)
#
#  The tooltip shown in the PSReadLine dropdown is the combined label string.

function Register-IdentityCompleter {
    Register-ArgumentCompleter -CommandName assumeIdentity -ParameterName User -ScriptBlock {
        param($cmd, $param, $word)
        $d   = Get-StoredData
        $map = Build-UserMap (Get-AllUsers $d)
        $low = $word.ToLower()

        $seen    = [System.Collections.Generic.HashSet[string]]::new()
        $results = [System.Collections.Generic.List[object]]::new()

        # Pass 1: username prefix
        foreach ($u in ($map | Where-Object { $_.Username -ilike "$word*" })) {
            if ($seen.Add($u.Username)) {
                $results.Add([System.Management.Automation.CompletionResult]::new(
                    $u.Username, $u.Username, 'ParameterValue', $u.Labels))
            }
        }

        # Pass 2: label keyword (skip if already matched by username)
        foreach ($u in ($map | Where-Object { $_.Labels.ToLower().Contains($low) })) {
            if ($seen.Add($u.Username)) {
                $results.Add([System.Management.Automation.CompletionResult]::new(
                    $u.Username, $u.Username, 'ParameterValue', $u.Labels))
            }
        }

        $results
    }

    Register-ArgumentCompleter -CommandName assumeIdentity -ParameterName Connection -ScriptBlock {
        param($cmd, $param, $word)
        $d = Get-StoredData
        (Get-AllConns $d) |
            Where-Object { $_ -ilike "$word*" } |
            ForEach-Object {
                [System.Management.Automation.CompletionResult]::new(
                    $_, $_, 'ParameterValue', $_)
            }
    }

    Write-Host "  Tab completion registered for assumeIdentity." -ForegroundColor Green
}

# -- Save new connection if not already known ---------------------------------

function Save-NewConnection($data, [string]$server) {
    $allConns = Get-AllConns $data
    if ($allConns | Where-Object { $_ -ieq $server }) { return }
    Write-Host "  Testing new connection $server..." -NoNewline
    $testStr  = "Server=$server;Database=logincheck;Integrated Security=SSPI;TrustServerCertificate=True;Connect Timeout=5;"
    $testConn = New-Object System.Data.SqlClient.SqlConnection $testStr
    try {
        $testConn.Open()
        $testConn.Close()
        Write-Host " OK" -ForegroundColor Green
        $data.CustomConnections += $server
        Save-StoredData $data
        Write-Host "  Saved '$server' for future sessions." -ForegroundColor Green
    } catch {
        Write-Host " FAILED: $($_.Exception.Message)" -ForegroundColor Red
    }
}

# -- Pre-flight identity check ------------------------------------------------

function Get-CurrentIdentity([string]$server, [string]$toAssume) {
    $lines = @(
        "DECLARE @ImposterLogin VARCHAR(35) = 'FNBA\' + '$IMPOSTER';"
        "DECLARE @ToAssumeLogin VARCHAR(35) = 'FNBA\' + '$toAssume';"
        "DECLARE @CurrentAssocId INT;"
        "DECLARE @TargetAssocId  INT;"
        "SELECT @CurrentAssocId = assoc_id FROM logincheck.fnba_reporting.associate_login WHERE domain_username = @ImposterLogin;"
        "SELECT @TargetAssocId  = assoc_id FROM logincheck.fnba_reporting.associate_login WHERE domain_username = @ToAssumeLogin;"
        "SELECT"
        "    CASE WHEN @CurrentAssocId = @TargetAssocId THEN 1 ELSE 0 END AS already_assuming,"
        "    al.password,"
        "    al.date_modified AS changed_at,"
        "    (SELECT domain_username FROM logincheck.fnba_reporting.associate_login WHERE assoc_id = al.assoc_id AND domain_username <> @ImposterLogin) AS acting_as_login,"
        "    per.first_name + ' ' + per.last_name AS acting_as_name"
        "FROM logincheck.fnba_reporting.associate_login al"
        "JOIN perdb.fnba.associate per ON per.assoc_id = al.assoc_id"
        "WHERE al.domain_username = @ImposterLogin;"
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
        if ($table.Rows.Count -gt 0) { return $table.Rows[0] }
    } catch {
        Write-Host "  ERROR during pre-flight check: $($_.Exception.Message)" -ForegroundColor Red
    } finally {
        $conn.Close()
    }
    return $null
}

# -- SQL -----------------------------------------------------------------------

function Invoke-AssumeIdentity([string]$server, [string]$toAssume) {
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

        # Declare variables
        "DECLARE @Imposter VARCHAR(35) = '$IMPOSTER';"
        "DECLARE @ToAssume VARCHAR(35) = '$toAssume';"
        "DECLARE @ImposterLogin VARCHAR(35) = 'FNBA\' + '$IMPOSTER';"
        "DECLARE @ToAssumeLogin VARCHAR(35) = 'FNBA\' + '$toAssume';"

        # Resolve assoc_ids to check current state and enable early exit
        "DECLARE @CurrentAssocId INT;"
        "DECLARE @TargetAssocId  INT;"
        "SELECT @CurrentAssocId = assoc_id FROM logincheck.fnba_reporting.associate_login WHERE domain_username = @ImposterLogin;"
        "SELECT @TargetAssocId  = assoc_id FROM logincheck.fnba_reporting.associate_login WHERE domain_username = @ToAssumeLogin;"



        # BEFORE: join associate on kbiesbrock's CURRENT assoc_id -- reveals who they are already acting as
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

        # AFTER: assoc_id is now the target's — join gives target's name
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
        Write-Host "  Connecting to $server..." -ForegroundColor DarkCyan
        $conn.Open()
        [void]$adapter.Fill($table)

        if ($table.Rows.Count -eq 0) {
            Write-Host "  ERROR: No rows returned." -ForegroundColor Red
            return
        }

        if ($table.Rows.Count -lt 2) {
            Write-Host "  ERROR: Unexpected result - expected before and after rows." -ForegroundColor Red
            return
        }

        $before  = $table.Rows[0]
        $after   = $table.Rows[1]
        $changed = $before["password"] -ne $after["password"]

        # If acting_as_login is null/empty in BEFORE, kbiesbrock was acting as themselves
        $beforeActingAsLogin = if ([string]::IsNullOrWhiteSpace($before["acting_as_login"])) { $before["imposter_login"] } else { $before["acting_as_login"] }
        $beforeActingAsName  = if ([string]::IsNullOrWhiteSpace($before["acting_as_login"])) { "self" }                    else { $before["acting_as_name"]  }

        Write-Host ""
        Write-Host "  Server  : $server"           -ForegroundColor DarkGray
        Write-Host "  Login   : FNBA\$IMPOSTER"    -ForegroundColor DarkGray
        Write-Host ""

        Write-Host "  BEFORE"                      -ForegroundColor DarkYellow
        Write-Host "    Acting as : $beforeActingAsLogin  ($beforeActingAsName)"
        Write-Host "    Password  : $($before["password"])"
        Write-Host "    Since     : $('{0:HH:mm:ss MM-dd-yyyy}' -f $before["changed_at"])"
        Write-Host ""

        $afterColor = if ($changed) { "Green" } else { "Red" }
        Write-Host "  AFTER"                       -ForegroundColor $afterColor
        Write-Host "    Acting as : $($after["acting_as_login"])  ($($after["acting_as_name"]))"
        Write-Host "    Password  : $($after["password"])"
        Write-Host "    Since     : $('{0:HH:mm:ss MM-dd-yyyy}' -f $after["changed_at"])"
        Write-Host ""

        if (-not $changed) {
            Write-Host "  WARNING: Password did not change - the identity switch may have failed." -ForegroundColor Red
        }

    } catch {
        Write-Host "  ERROR: $($_.Exception.Message)" -ForegroundColor Red
    } finally {
        $conn.Close()
    }
}

# -- Entry point ---------------------------------------------------------------

if ($LoadOnly) { return }

$data = Get-StoredData

$toAssume = Resolve-User $data $User
$server   = Resolve-Conn $data $Connection

if (-not $toAssume) {
    Write-Host "  Unknown user: '$User'" -ForegroundColor Red
    Write-Host "  Usage: assumeIdentity <user> <connection>"
    Write-Host "  Tab-complete both arguments. Label search also works for <user>."
    exit 1
}

if (-not $server) {
    Write-Host "  Unknown connection: '$Connection'" -ForegroundColor Red
    Write-Host "  Usage: assumeIdentity <user> <connection>"
    Write-Host "  Tab-complete both arguments."
    exit 1
}

Save-NewConnection $data $server

# Pre-flight: check current identity before prompting
$current = Get-CurrentIdentity -server $server -toAssume $toAssume

if ($null -ne $current -and [int]$current["already_assuming"] -eq 1) {
    $actingAsLogin = if ([string]::IsNullOrWhiteSpace($current["acting_as_login"])) { "FNBA\$IMPOSTER" } else { $current["acting_as_login"] }
    $actingAsName  = if ([string]::IsNullOrWhiteSpace($current["acting_as_login"])) { "self" }           else { $current["acting_as_name"]  }
    Write-Host ""
    Write-Host "  Server  : $server"        -ForegroundColor DarkGray
    Write-Host "  Login   : FNBA\$IMPOSTER" -ForegroundColor DarkGray
    Write-Host ""
    Write-Host "  Already acting as this identity - no change needed." -ForegroundColor Yellow
    Write-Host ""
    Write-Host "  CURRENT" -ForegroundColor Green
    Write-Host "    Acting as : $actingAsLogin  ($actingAsName)"
    Write-Host "    Password  : $($current["password"])"
    Write-Host "    Since     : $('{0:HH:mm:ss MM-dd-yyyy}' -f $current["changed_at"])"
    Write-Host ""
    exit 0
}

Write-Host ""
Write-Host "  Imposter : FNBA\$IMPOSTER" -ForegroundColor DarkGray
Write-Host "  Assume   : $toAssume"      -ForegroundColor Yellow
Write-Host "  Server   : $server"        -ForegroundColor Yellow
Write-Host ""
$confirm = Read-Host "  Proceed? [Y/n]"
if ($confirm -ine "y" -and $confirm -ne "") { Write-Host "Cancelled." -ForegroundColor Yellow; exit 0 }

Invoke-AssumeIdentity -server $server -toAssume $toAssume

<#
================================================================================
 INSTALLATION AND SETUP
================================================================================

 STEP 1 - Check / install PowerShell 7
 ---------------------------------------
 This script requires PowerShell 7 (pwsh). Check your version:

   $PSVersionTable.PSVersion

 If the major version is less than 7, install it (no admin required):

   winget install Microsoft.PowerShell

 PS7 installs alongside PS5 and does not replace it.


 STEP 2 - Save the script
 -------------------------
 Save this file to a permanent location, e.g.:
   C:\Users\kbiesbrock\scripts\assumeIdentity.ps1


 STEP 3 - Allow local scripts to run (one-time)
 ------------------------------------------------
 In PowerShell 7 (pwsh):

   Set-ExecutionPolicy -Scope CurrentUser RemoteSigned

 Press Y to confirm.


 STEP 4 - Add to your PowerShell profile
 -----------------------------------------
 Open your PS7 profile:

   notepad $PROFILE

 If it does not exist yet:

   New-Item -Path $PROFILE -ItemType File -Force

 Add these lines in this exact order:

   . "C:\Users\kbiesbrock\scripts\assumeIdentity.ps1" -LoadOnly

   function assumeIdentity {
       param(
           [Parameter(Position=0)][string]$User,
           [Parameter(Position=1)][string]$Connection
       )
       pwsh -NoProfile -File "C:\Users\kbiesbrock\scripts\assumeIdentity.ps1" $User $Connection
   }

   Register-IdentityCompleter

 Why this order matters:
   1. Dot-source loads all helper functions, including Register-IdentityCompleter
   2. The wrapper function must declare $User and $Connection as named parameters
      so that Register-ArgumentCompleter has something to attach completers to
   3. Register-IdentityCompleter wires the completers to those parameter names

 Save and reload:

   . $PROFILE

 You should see: Tab completion registered for assumeIdentity.


 STEP 5 - Add to PATH (optional)
 ---------------------------------
 Only needed if you want to call assumeIdentity from cmd.exe or Run (Win+R).

   1. Start -> search "Edit environment variables for your account"
   2. Select Path -> Edit -> New
   3. Add: C:\Users\kbiesbrock\scripts
   4. OK all the way out, open a new terminal.


================================================================================
 USAGE
================================================================================

   assumeIdentity <user> <connection>

 Both arguments are positional and tab-complete.


 USER ARGUMENT - tab-complete by username prefix OR label keyword:

   cpat<TAB>       -> cpatterson        [Application Rights | Rosebud]
   ww<TAB>         -> wwessels          [Underwriting]
   batch<TAB>      -> clacroix, mkeller, aguggemmos, pbenson, ahanson
   underw<TAB>     -> ajenks, sfortino, wwessels
   account<TAB>    -> yferguson, jbouck


 CONNECTION ARGUMENT - tab-complete by hostname prefix:

   kbi<TAB>        -> dsqlkbiesbrock.fnba-dev.network
   ale<TAB>        -> dsqlaleroy.fnba-dev.network
   mel<TAB>        -> meleagris.fnba.com


 FULL EXAMPLES:

   assumeIdentity cpat<TAB> kbi<TAB>    -> runs as cpatterson on dsqlkbiesbrock
   assumeIdentity batch<TAB>            -> dropdown of all Batch users
   assumeIdentity cpatterson dsqlkbiesbrock.fnba-dev.network


 NEW CONNECTIONS:
   Type any hostname as the second argument. If the connection succeeds,
   it is saved automatically to ~\.assumeIdentity.json and offered in
   future tab completions.

================================================================================
#>