<#
.SYNOPSIS
  Diagnose and repair a library database whose schema is newer than the build.

.DESCRIPTION
  emusic applies additive schema migrations tracked in `PRAGMA user_version`.
  If you run a newer/dev build (for example a branch that has not landed yet),
  it migrates `%LOCALAPPDATA%\emusic\library.db` past the version the installed
  build understands. The installed build then refuses to open the file and
  silently falls back to an in-memory store, so the app opens with an empty
  library even though the data is intact.

  This script compares the database's `user_version` with the version the
  current source tree expects (parsed from
  `crates/library/src/store/schema.rs`, so it follows the code) and reports
  any schema objects the target build does not know about.

  With -Repair it:
    1. checkpoints and backs up the database next to the original,
    2. drops the tables, indexes and columns introduced by migrations newer
       than the target,
    3. sets `user_version` back to the target and vacuums the file.

  Afterwards the current build opens the database normally, and a future run
  of the newer build re-applies its migrations cleanly.

  Uses the SQLite shipped with Windows (`winsqlite3.dll`); no external tools.

.PARAMETER Path
  Database to inspect/repair. Defaults to `%LOCALAPPDATA%\emusic\library.db`.

.PARAMETER TargetVersion
  Schema version to bring the database back to. Defaults to the version the
  current source tree expects.

.PARAMETER Repair
  Actually modify the database. Without it the script only reports.

.PARAMETER Yes
  Skip the confirmation prompt (for unattended use).

.EXAMPLE
  .\scripts\repair-db.ps1
  Report the database version against the current tree.

.EXAMPLE
  .\scripts\repair-db.ps1 -Repair
  Back up and downgrade the default database to the current tree's version.
#>
[CmdletBinding()]
param(
    [string]$Path = (Join-Path $env:LOCALAPPDATA 'emusic\library.db'),
    [int]$TargetVersion = 0,
    [switch]$Repair,
    [switch]$Yes
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

Add-Type @'
using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;
using System.Text;

public static class SqliteCli {
    const string DLL = "winsqlite3.dll";
    const int SQLITE_OK = 0;
    const int SQLITE_ROW = 100;
    const int SQLITE_DONE = 101;

    [DllImport(DLL, CallingConvention = CallingConvention.Cdecl)]
    static extern int sqlite3_open(byte[] filename, out IntPtr db);
    [DllImport(DLL, CallingConvention = CallingConvention.Cdecl)]
    static extern int sqlite3_close(IntPtr db);
    [DllImport(DLL, CallingConvention = CallingConvention.Cdecl)]
    static extern int sqlite3_busy_timeout(IntPtr db, int ms);
    [DllImport(DLL, CallingConvention = CallingConvention.Cdecl)]
    static extern int sqlite3_exec(IntPtr db, byte[] sql, IntPtr callback, IntPtr arg, out IntPtr errmsg);
    [DllImport(DLL, CallingConvention = CallingConvention.Cdecl)]
    static extern void sqlite3_free(IntPtr p);
    [DllImport(DLL, CallingConvention = CallingConvention.Cdecl)]
    static extern int sqlite3_prepare_v2(IntPtr db, byte[] sql, int nByte, out IntPtr stmt, IntPtr tail);
    [DllImport(DLL, CallingConvention = CallingConvention.Cdecl)]
    static extern int sqlite3_step(IntPtr stmt);
    [DllImport(DLL, CallingConvention = CallingConvention.Cdecl)]
    static extern IntPtr sqlite3_column_text(IntPtr stmt, int col);
    [DllImport(DLL, CallingConvention = CallingConvention.Cdecl)]
    static extern int sqlite3_column_bytes(IntPtr stmt, int col);
    [DllImport(DLL, CallingConvention = CallingConvention.Cdecl)]
    static extern int sqlite3_column_count(IntPtr stmt);
    [DllImport(DLL, CallingConvention = CallingConvention.Cdecl)]
    static extern int sqlite3_finalize(IntPtr stmt);
    [DllImport(DLL, CallingConvention = CallingConvention.Cdecl)]
    static extern IntPtr sqlite3_errmsg(IntPtr db);
    [DllImport(DLL, CallingConvention = CallingConvention.Cdecl)]
    static extern IntPtr sqlite3_libversion();

    static byte[] Utf8(string s) { return Encoding.UTF8.GetBytes(s + "\0"); }

    static string PtrUtf8(IntPtr p) {
        if (p == IntPtr.Zero) return null;
        int len = 0;
        while (Marshal.ReadByte(p, len) != 0) len++;
        var buf = new byte[len];
        Marshal.Copy(p, buf, 0, len);
        return Encoding.UTF8.GetString(buf);
    }

    static string Error(IntPtr db) {
        return db == IntPtr.Zero ? "unknown error" : PtrUtf8(sqlite3_errmsg(db));
    }

    public static string Version() { return PtrUtf8(sqlite3_libversion()); }

    public static IntPtr Open(string path) {
        IntPtr db;
        int rc = sqlite3_open(Utf8(path), out db);
        if (rc != SQLITE_OK) {
            string msg = db == IntPtr.Zero ? ("rc=" + rc) : Error(db);
            if (db != IntPtr.Zero) sqlite3_close(db);
            throw new Exception("could not open " + path + ": " + msg);
        }
        sqlite3_busy_timeout(db, 5000);
        return db;
    }

    public static void Close(IntPtr db) { if (db != IntPtr.Zero) sqlite3_close(db); }

    public static void Exec(IntPtr db, string sql) {
        IntPtr errmsg;
        int rc = sqlite3_exec(db, Utf8(sql), IntPtr.Zero, IntPtr.Zero, out errmsg);
        if (rc != SQLITE_OK) {
            string msg = errmsg == IntPtr.Zero ? ("rc=" + rc) : PtrUtf8(errmsg);
            if (errmsg != IntPtr.Zero) sqlite3_free(errmsg);
            throw new Exception("SQL failed: " + msg + "  [" + sql + "]");
        }
    }

    public static List<string[]> Rows(IntPtr db, string sql) {
        IntPtr stmt;
        int rc = sqlite3_prepare_v2(db, Utf8(sql), -1, out stmt, IntPtr.Zero);
        if (rc != SQLITE_OK) throw new Exception("prepare failed: " + Error(db) + "  [" + sql + "]");
        try {
            int n = sqlite3_column_count(stmt);
            var rows = new List<string[]>();
            while ((rc = sqlite3_step(stmt)) == SQLITE_ROW) {
                var row = new string[n];
                for (int i = 0; i < n; i++) {
                    IntPtr p = sqlite3_column_text(stmt, i);
                    if (p == IntPtr.Zero) { row[i] = null; continue; }
                    int bytes = sqlite3_column_bytes(stmt, i);
                    var buf = new byte[bytes];
                    Marshal.Copy(p, buf, 0, bytes);
                    row[i] = Encoding.UTF8.GetString(buf);
                }
                rows.Add(row);
            }
            if (rc != SQLITE_DONE) throw new Exception("query failed: " + Error(db));
            return rows;
        } finally {
            sqlite3_finalize(stmt);
        }
    }

    public static string Scalar(IntPtr db, string sql) {
        var rows = Rows(db, sql);
        return rows.Count == 0 ? null : rows[0][0];
    }
}
'@

$RepoRoot = Split-Path -Parent $PSScriptRoot
$SchemaFile = Join-Path $RepoRoot 'crates\library\src\store\schema.rs'

function Get-Migrations {
    if (-not (Test-Path -LiteralPath $SchemaFile)) {
        throw "schema.rs not found at $SchemaFile; run this script from a checkout of emusic."
    }
    $text = Get-Content -LiteralPath $SchemaFile -Raw
    if ($text -notmatch 'CURRENT_VERSION:\s*i64\s*=\s*(\d+)') {
        throw "could not parse CURRENT_VERSION from $SchemaFile"
    }
    $supported = [int]$Matches[1]
    $sql = @(
        [regex]::Matches($text, 'r"(?<sql>[^"]*)"', [System.Text.RegularExpressions.RegexOptions]::Singleline) |
            ForEach-Object { $_.Groups['sql'].Value }
    )
    if ($sql.Count -lt $supported) {
        throw "parsed $($sql.Count) migrations but CURRENT_VERSION is $supported"
    }
    [pscustomobject]@{ Supported = $supported; Sql = $sql }
}

function Get-Schema {
    param([IntPtr]$Db)
    $tables = @{}
    foreach ($table in [SqliteCli]::Rows($Db, "SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%'")) {
        $name = [string]$table[0]
        $columns = [System.Collections.Generic.HashSet[string]]::new([System.StringComparer]::OrdinalIgnoreCase)
        foreach ($column in [SqliteCli]::Rows($Db, "PRAGMA table_info('$name')")) {
            [void]$columns.Add([string]$column[1])
        }
        $tables[$name] = $columns
    }
    $indexes = [System.Collections.Generic.HashSet[string]]::new([System.StringComparer]::OrdinalIgnoreCase)
    foreach ($index in [SqliteCli]::Rows($Db, "SELECT name FROM sqlite_master WHERE type='index' AND name NOT LIKE 'sqlite_%'")) {
        [void]$indexes.Add([string]$index[0])
    }
    [pscustomobject]@{ Tables = $tables; Indexes = $indexes }
}

function Get-ExtraObjects {
    param($Live, $Target)
    $tables = @($Live.Tables.Keys | Where-Object { -not $Target.Tables.ContainsKey($_) } | Sort-Object)
    $indexes = @($Live.Indexes | Where-Object { -not $Target.Indexes.Contains($_) } | Sort-Object)
    $columns = @()
    foreach ($table in $Live.Tables.Keys) {
        if (-not $Target.Tables.ContainsKey($table)) { continue }
        foreach ($column in $Live.Tables[$table]) {
            if (-not $Target.Tables[$table].Contains($column)) {
                $columns += [pscustomobject]@{ Table = $table; Column = $column }
            }
        }
    }
    [pscustomobject]@{ Tables = $tables; Indexes = $indexes; Columns = $columns }
}

$migrations = Get-Migrations
$supported = $migrations.Supported
$target = if ($TargetVersion -gt 0) { $TargetVersion } else { $supported }
if ($target -lt 1 -or $target -gt $migrations.Sql.Count) {
    throw "target version $target is out of range (1..$($migrations.Sql.Count))"
}

Write-Host "SQLite:               $([SqliteCli]::Version())"
Write-Host "Source version:       $supported  (crates/library/src/store/schema.rs)"
Write-Host "Target version:       $target"
Write-Host "Database:             $Path"

if (-not (Test-Path -LiteralPath $Path)) {
    throw "database not found: $Path"
}

$db = [SqliteCli]::Open($Path)
try {
    [SqliteCli]::Exec($db, 'PRAGMA wal_checkpoint(TRUNCATE)')
    $version = [int][SqliteCli]::Scalar($db, 'PRAGMA user_version')
    Write-Host "Database version:     $version"
    Write-Host ''

    if ($version -eq $target) {
        Write-Host 'OK: the database matches the target schema; nothing to do.' -ForegroundColor Green
        return
    }

    if ($version -lt $target) {
        Write-Host "The database is older than the target; run the target build to migrate it forward." -ForegroundColor Yellow
        return
    }

    $targetDb = Join-Path ([System.IO.Path]::GetTempPath()) ("emusic-target-$([guid]::NewGuid()).db")
    try {
        $tdb = [SqliteCli]::Open($targetDb)
        try {
            for ($i = 0; $i -lt $target; $i++) { [SqliteCli]::Exec($tdb, $migrations.Sql[$i]) }
            $targetSchema = Get-Schema -Db $tdb
        } finally {
            [SqliteCli]::Close($tdb)
        }
        $liveSchema = Get-Schema -Db $db
        $extra = Get-ExtraObjects -Live $liveSchema -Target $targetSchema

        if ($extra.Tables.Count -eq 0 -and $extra.Indexes.Count -eq 0 -and $extra.Columns.Count -eq 0) {
            Write-Host "The database is version $version but has no objects beyond version $target." -ForegroundColor Yellow
            Write-Host "A newer build added only a version bump; setting user_version back is enough."
        } else {
            Write-Host "Objects newer than version ${target}:" -ForegroundColor Yellow
            foreach ($table in $extra.Tables) { Write-Host "  table   $table" }
            foreach ($index in $extra.Indexes) { Write-Host "  index   $index" }
            foreach ($column in $extra.Columns) { Write-Host "  column  $($column.Table).$($column.Column)" }
        }
        Write-Host ''

        if (-not $Repair) {
            Write-Host "Re-run with -Repair to back up the database, drop these objects and set user_version to $target." -ForegroundColor Cyan
            exit 2
        }

        if (-not $Yes) {
            $answer = Read-Host "Back up and repair $Path? [y/N]"
            if ($answer -notmatch '^(y|yes)$') {
                Write-Host 'Aborted.'
                return
            }
        }

        $timestamp = Get-Date -Format 'yyyyMMdd-HHmmss'
        $backup = "$Path.v$version-backup-$timestamp"
        Copy-Item -LiteralPath $Path -Destination $backup -Force
        Write-Host "Backup:               $backup" -ForegroundColor Green

        [SqliteCli]::Exec($db, 'BEGIN')
        try {
            foreach ($index in $extra.Indexes) { [SqliteCli]::Exec($db, "DROP INDEX IF EXISTS ""$index""") }
            foreach ($column in $extra.Columns) { [SqliteCli]::Exec($db, "ALTER TABLE ""$($column.Table)"" DROP COLUMN ""$($column.Column)""") }
            foreach ($table in $extra.Tables) { [SqliteCli]::Exec($db, "DROP TABLE IF EXISTS ""$table""") }
            [SqliteCli]::Exec($db, "PRAGMA user_version = $target")
            [SqliteCli]::Exec($db, 'COMMIT')
        } catch {
            [SqliteCli]::Exec($db, 'ROLLBACK')
            throw
        }

        [SqliteCli]::Exec($db, 'VACUUM')
        $integrity = [SqliteCli]::Scalar($db, 'PRAGMA integrity_check')
        $now = [int][SqliteCli]::Scalar($db, 'PRAGMA user_version')
        Write-Host "New version:          $now"
        Write-Host "Integrity:            $integrity"
        if ($integrity -ne 'ok' -or $now -ne $target) {
            throw "repair did not verify; restore from $backup"
        }
        Write-Host 'Repair complete. Start emusic; if it still looks empty, restore the backup and open an issue.' -ForegroundColor Green
    } finally {
        if (Test-Path -LiteralPath $targetDb) { Remove-Item -LiteralPath $targetDb -Force }
    }
} finally {
    [SqliteCli]::Close($db)
}
