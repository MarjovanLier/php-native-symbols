// ---------------------------------------------------------------------------
// Function tables (M1/M2). Names are lowercase lookup keys.
// ---------------------------------------------------------------------------

/// Extensions known to be only conditionally compiled across the reflection
/// builds, so the diff misplaces their ancient functions in-range. Reviewed: if
/// added-artefact correction ever fires for an extension not listed here,
/// generation fails so the new case gets a human look before the data changes.
pub(crate) const FUNCTION_ADDED_ARTIFACT_EXTENSIONS: &[&str] = &["odbc", "tidy", "zip"];

/// Reviewed per-symbol `added` overrides, each resolved against the PHP manual
/// (a fact, corroborated by PHPCompatibility) for functions the diff would
/// otherwise mis-date. `Some(v)` pins an in-range version; `None` marks a
/// function that predates the 7.4 floor. These are the recorded resolutions the
/// mandatory cross-check demands, so no minimum-version ships as a guess. Names
/// are lookup keys (lowercase).
pub(crate) const FUNCTION_ADDED_OVERRIDES: &[(&str, Option<(u8, u8)>)] = &[
    // odbc connection-string helpers: genuinely new in 8.2 (PHP manual,
    // corroborated by PHPCompatibility). phpstorm-stubs carries no @since and
    // only compiled odbc in the 8.3 build, so the bare diff would say 8.3.
    ("odbc_connection_string_is_quoted", Some((8, 2))),
    ("odbc_connection_string_quote", Some((8, 2))),
    ("odbc_connection_string_should_quote", Some((8, 2))),
    // IntlTimeZone Windows-ID procedural functions: added 7.1 (PHP manual,
    // PHPCompatibility), so they predate the 7.4 floor. The intl extension is
    // built at the floor but only exposes these from 8.0, so the diff says 8.0.
    ("intltz_get_windows_id", None),
    ("intltz_get_id_for_windows_id", None),
];

/// Extensions whose functions disappear from the late reflection builds only
/// because the extension was not compiled there, not because PHP removed them
/// (they remain in core). A presence-shape removal for one of these, when
/// PHPCompatibility is silent, is a build artefact -> `removed: None`. Reviewed:
/// a silent disappearance for an extension not listed here fails generation, so
/// a genuine future removal cannot slip through as "still available". Distinct
/// from (and larger than) [`FUNCTION_ADDED_ARTIFACT_EXTENSIONS`] because more
/// extensions drop out of the late builds than are mis-dated forward at the
/// floor. `imap` and `pspell` are deliberately absent: they were genuinely
/// unbundled at 8.4, so PHPCompatibility confirms them and they take the
/// confirmed-removal path.
pub(crate) const FUNCTION_REMOVED_ARTIFACT_EXTENSIONS: &[&str] =
    &["exif", "ftp", "gettext", "odbc", "tidy", "zip"];

/// Reviewed per-symbol `removed` overrides. `Some(v)` pins a removal version,
/// `None` forces "not removed". Empty: every current removal is confirmed by
/// PHPCompatibility's `true`-version and every silent disappearance is a reviewed
/// build artefact, so none is needed. The slot exists so a future genuine
/// removal PHPCompatibility has not yet recorded has a reviewed home (it must
/// still agree with PHPCompatibility where the latter has an opinion).
pub(crate) const FUNCTION_REMOVED_OVERRIDES: &[(&str, Option<(u8, u8)>)] = &[];

/// Reviewed per-symbol `deprecated` overrides, each a PHP-manual fact that must
/// equal PHPCompatibility's `false`-version. They fill two gaps the cache cannot
/// date: a function already deprecated at the 7.4 floor (the cache clamps it to
/// 7.4 or, for `each`, never flags it) and one whose extension is compiled too
/// late to show the real flag (`odbc_result_all`). `Some(v)` pins the real
/// version. Names are lowercase lookup keys.
pub(crate) const FUNCTION_DEPRECATED_OVERRIDES: &[(&str, Option<(u8, u8)>)] = &[
    // Deprecated before the 7.4 floor (PHP manual, corroborated by
    // PHPCompatibility's false-version); all also removed at 8.0.
    ("ldap_sort", Some((7, 0))),
    ("create_function", Some((7, 2))),
    ("each", Some((7, 2))),
    ("gmp_random", Some((7, 2))),
    ("jpeg2wbmp", Some((7, 2))),
    ("png2wbmp", Some((7, 2))),
    ("read_exif_data", Some((7, 2))),
    ("fgetss", Some((7, 3))),
    ("gzgetss", Some((7, 3))),
    ("image2wbmp", Some((7, 3))),
    ("mbereg", Some((7, 3))),
    ("mbereg_match", Some((7, 3))),
    ("mbereg_replace", Some((7, 3))),
    ("mbereg_search", Some((7, 3))),
    ("mbereg_search_getpos", Some((7, 3))),
    ("mbereg_search_getregs", Some((7, 3))),
    ("mbereg_search_init", Some((7, 3))),
    ("mbereg_search_pos", Some((7, 3))),
    ("mbereg_search_regs", Some((7, 3))),
    ("mbereg_search_setpos", Some((7, 3))),
    ("mberegi", Some((7, 3))),
    ("mberegi_replace", Some((7, 3))),
    ("mbregex_encoding", Some((7, 3))),
    ("mbsplit", Some((7, 3))),
    // odbc is compiled too late in the caches to show the 8.1 flag; deprecated
    // 8.1 (PHP manual, PHPCompatibility false). Not removed (still core).
    ("odbc_result_all", Some((8, 1))),
];

/// Functions PHPCompatibility records a `false`-version for that this crate
/// deliberately does not model as deprecated, with the reviewed reason. The
/// reconciliation gate skips them; each must keep `deprecated: None` so an
/// exclusion can never hide a real deprecation.
pub(crate) const FUNCTION_DEPRECATION_EXCLUSIONS: &[(&str, &str)] = &[(
    "dl",
    "deprecation is SAPI-conditional and pre-floor (5.3); not modelled as a global function deprecation",
)];

/// Editorial deprecation successors for functions, the only hand-curated values
/// in the function table. Sourced from the PHP manual deprecation page and the
/// stub `@deprecated` message as terse canonical labels (a function, a method,
/// or a short construct hint), never copied prose. Present only where a single
/// clear successor exists; a deprecation with no single replacement is simply
/// absent here. Each name must end up `deprecated: Some(..)` or generation fails
/// (stale curation), and a successor may not be the deprecated function itself.
/// Names are lowercase lookup keys.
pub(crate) const FUNCTION_REPLACEMENTS: &[(&str, &str)] = &[
    ("create_function", "an anonymous function"),
    ("date_sunrise", "date_sun_info()"),
    ("date_sunset", "date_sun_info()"),
    ("each", "a foreach loop"),
    ("gmstrftime", "IntlDateFormatter::format()"),
    ("image2wbmp", "imagewbmp()"),
    ("is_real", "is_float()"),
    ("mbereg", "mb_ereg()"),
    ("mbereg_match", "mb_ereg_match()"),
    ("mbereg_replace", "mb_ereg_replace()"),
    ("mbereg_search", "mb_ereg_search()"),
    ("mbereg_search_getpos", "mb_ereg_search_getpos()"),
    ("mbereg_search_getregs", "mb_ereg_search_getregs()"),
    ("mbereg_search_init", "mb_ereg_search_init()"),
    ("mbereg_search_pos", "mb_ereg_search_pos()"),
    ("mbereg_search_regs", "mb_ereg_search_regs()"),
    ("mbereg_search_setpos", "mb_ereg_search_setpos()"),
    ("mberegi", "mb_eregi()"),
    ("mberegi_replace", "mb_eregi_replace()"),
    ("mbregex_encoding", "mb_regex_encoding()"),
    ("mbsplit", "mb_split()"),
    ("mhash", "hash()"),
    ("money_format", "NumberFormatter::formatCurrency()"),
    ("mysqli_execute", "mysqli_stmt_execute()"),
    ("read_exif_data", "exif_read_data()"),
    ("restore_include_path", "ini_restore('include_path')"),
    ("socket_set_timeout", "stream_set_timeout()"),
    ("strftime", "IntlDateFormatter::format()"),
    ("utf8_decode", "mb_convert_encoding()"),
    ("utf8_encode", "mb_convert_encoding()"),
    // postgres deprecated aliases -> canonical underscore spellings.
    ("pg_clientencoding", "pg_client_encoding()"),
    ("pg_cmdtuples", "pg_affected_rows()"),
    ("pg_errormessage", "pg_last_error()"),
    ("pg_fieldisnull", "pg_field_is_null()"),
    ("pg_fieldname", "pg_field_name()"),
    ("pg_fieldnum", "pg_field_num()"),
    ("pg_fieldprtlen", "pg_field_prtlen()"),
    ("pg_fieldsize", "pg_field_size()"),
    ("pg_fieldtype", "pg_field_type()"),
    ("pg_freeresult", "pg_free_result()"),
    ("pg_getlastoid", "pg_last_oid()"),
    ("pg_loclose", "pg_lo_close()"),
    ("pg_locreate", "pg_lo_create()"),
    ("pg_loexport", "pg_lo_export()"),
    ("pg_loimport", "pg_lo_import()"),
    ("pg_loopen", "pg_lo_open()"),
    ("pg_loread", "pg_lo_read()"),
    ("pg_loreadall", "pg_lo_read_all()"),
    ("pg_lounlink", "pg_lo_unlink()"),
    ("pg_lowrite", "pg_lo_write()"),
    ("pg_numfields", "pg_num_fields()"),
    ("pg_numrows", "pg_num_rows()"),
    ("pg_result", "pg_fetch_result()"),
    ("pg_setclientencoding", "pg_set_client_encoding()"),
    // procedural zip API -> the ZipArchive class (stub @deprecated says so).
    ("zip_close", "ZipArchive"),
    ("zip_entry_close", "ZipArchive"),
    ("zip_entry_compressedsize", "ZipArchive"),
    ("zip_entry_compressionmethod", "ZipArchive"),
    ("zip_entry_filesize", "ZipArchive"),
    ("zip_entry_name", "ZipArchive"),
    ("zip_entry_open", "ZipArchive"),
    ("zip_entry_read", "ZipArchive"),
    ("zip_open", "ZipArchive"),
    ("zip_read", "ZipArchive"),
];

/// PHP-CS-Fixer `NativeFunctionInvocationFixer` `@compiler_optimized` set:
/// functions the Zend engine compiles to a special opcode. Taken verbatim from
/// `src/Fixer/FunctionNotation/NativeFunctionInvocationFixer.php` at
/// [`PHP_CS_FIXER_TAG`] (MIT licence, attributed in NOTICE). Names are
/// lowercase, matching the generated lookup key.
pub(crate) const COMPILER_OPTIMIZED: &[&str] = &[
    "array_key_exists",
    "array_slice",
    "assert",
    "boolval",
    "call_user_func",
    "call_user_func_array",
    "chr",
    "constant",
    "count",
    "define",
    "defined",
    "dirname",
    "doubleval",
    "extension_loaded",
    "floatval",
    "func_get_args",
    "func_num_args",
    "function_exists",
    "get_called_class",
    "get_class",
    "gettype",
    "in_array",
    "ini_get",
    "intval",
    "is_array",
    "is_bool",
    "is_callable",
    "is_double",
    "is_float",
    "is_int",
    "is_integer",
    "is_long",
    "is_null",
    "is_object",
    "is_real",
    "is_resource",
    "is_scalar",
    "is_string",
    "ord",
    "sizeof",
    "sprintf",
    "strlen",
    "strval",
];

// ---------------------------------------------------------------------------
// Constant tables (M3). Constant names are CASE-SENSITIVE: keys are exact bytes
// (one leading `\` stripped), never lowercased.
// ---------------------------------------------------------------------------

/// Extensions whose constants the diff mis-dates forward because the extension
/// is absent at the 7.4 floor build (so its in-range diff value is a build
/// artefact for a pre-floor constant -> `None`). `PDO` carries the bridge
/// constant `PDO_ODBC_TYPE`, which the diff corrects to `None` and a reviewed
/// override then pins to its real 8.3. Reviewed allowlist: a correction for an
/// extension not listed here fails generation.
pub(crate) const CONSTANT_ADDED_ARTIFACT_EXTENSIONS: &[&str] = &["PDO", "odbc", "tidy", "xsl"];

/// Reviewed per-symbol constant `added` overrides (PHP-manual facts, each
/// corroborated by PHPCompatibility's NewConstantsSniff). The 28 `TIDY_TAG_*`
/// HTML5 tag constants were added in 7.4 (PHP manual and stub `@since` both say
/// 7.4), but tidy is only compiled in the 8.0..8.3 builds, so the diff mis-dates
/// them to 8.0. `PDO_ODBC_TYPE` (8.3) and `PGSQL_TRACE_SUPPRESS_TIMESTAMPS`
/// (8.3) are real in-range additions the late-compiled builds mis-date.
pub(crate) const CONSTANT_ADDED_OVERRIDES: &[(&str, Option<(u8, u8)>)] = &[
    ("TIDY_TAG_ARTICLE", Some((7, 4))),
    ("TIDY_TAG_ASIDE", Some((7, 4))),
    ("TIDY_TAG_AUDIO", Some((7, 4))),
    ("TIDY_TAG_BDI", Some((7, 4))),
    ("TIDY_TAG_CANVAS", Some((7, 4))),
    ("TIDY_TAG_COMMAND", Some((7, 4))),
    ("TIDY_TAG_DATALIST", Some((7, 4))),
    ("TIDY_TAG_DETAILS", Some((7, 4))),
    ("TIDY_TAG_DIALOG", Some((7, 4))),
    ("TIDY_TAG_FIGCAPTION", Some((7, 4))),
    ("TIDY_TAG_FIGURE", Some((7, 4))),
    ("TIDY_TAG_FOOTER", Some((7, 4))),
    ("TIDY_TAG_HEADER", Some((7, 4))),
    ("TIDY_TAG_HGROUP", Some((7, 4))),
    ("TIDY_TAG_MAIN", Some((7, 4))),
    ("TIDY_TAG_MARK", Some((7, 4))),
    ("TIDY_TAG_MENUITEM", Some((7, 4))),
    ("TIDY_TAG_METER", Some((7, 4))),
    ("TIDY_TAG_NAV", Some((7, 4))),
    ("TIDY_TAG_OUTPUT", Some((7, 4))),
    ("TIDY_TAG_PROGRESS", Some((7, 4))),
    ("TIDY_TAG_SECTION", Some((7, 4))),
    ("TIDY_TAG_SOURCE", Some((7, 4))),
    ("TIDY_TAG_SUMMARY", Some((7, 4))),
    ("TIDY_TAG_TEMPLATE", Some((7, 4))),
    ("TIDY_TAG_TIME", Some((7, 4))),
    ("TIDY_TAG_TRACK", Some((7, 4))),
    ("TIDY_TAG_VIDEO", Some((7, 4))),
    ("PDO_ODBC_TYPE", Some((8, 3))),
    ("PGSQL_TRACE_SUPPRESS_TIMESTAMPS", Some((8, 3))),
];

/// Reviewed per-symbol constant `removed` overrides. `OPENSSL_SSLV23_PADDING`
/// disappears from the 8.1 build because OpenSSL 3.0 dropped the underlying
/// `RSA_SSLV23_PADDING`; the openssl extension itself is present in every build
/// (its constant count grows 47 -> 71 across the range), so this is a linked-
/// library artefact, not a PHP removal, and PHPCompatibility is silent. It must
/// not go in [`CONSTANT_REMOVED_ARTIFACT_EXTENSIONS`] (that would mask a genuine
/// future openssl removal), so it is pinned here to `None`.
pub(crate) const CONSTANT_REMOVED_OVERRIDES: &[(&str, Option<(u8, u8)>)] =
    &[("OPENSSL_SSLV23_PADDING", None)];

/// Extensions whose constants vanish wholesale from a late reflection build
/// because the extension was not compiled there, not because PHP removed them.
/// `tidy` (8.0..8.3 only), `odbc` (8.3 only) and `xsl` (8.0..8.3 only) appear
/// late and then drop; `exif` and `ftp` are present through 8.4 and drop at 8.5.
/// A PHPCompatibility-silent disappearance for one of these is a build artefact
/// -> `removed: None`; a silent disappearance outside this allowlist fails
/// generation.
pub(crate) const CONSTANT_REMOVED_ARTIFACT_EXTENSIONS: &[&str] =
    &["exif", "ftp", "odbc", "tidy", "xsl"];

/// Editorial constant deprecation versions: the sole source of constant
/// `deprecated`. The reflection caches carry no `isDeprecated` for constants and
/// PHPCompatibility ships no constant-deprecation sniff, so there is neither a
/// machine source nor a second structured source to cross-check. These are
/// reviewed PHP-manual facts, each corroborated by the stub phpDoc `@deprecated`
/// where present (the filter constants) and fact-locked in tests. Treated as
/// editorial, exactly like [`Replacements`]: every name must exist in the table
/// or generation fails (stale curation). Names are exact-case lookup keys.
pub(crate) const CONSTANT_DEPRECATIONS: &[(&str, (u8, u8))] = &[
    // E_STRICT: deprecated 8.4 (RFC: Deprecate E_STRICT, PHP 8.4). Not removed.
    ("E_STRICT", (8, 4)),
    // FILTER_VALIDATE_URL flag aliases: deprecated 7.3, removed 8.0 (stub
    // @deprecated 7.3 / @removed 8.0 in filter/filter.php).
    ("FILTER_FLAG_HOST_REQUIRED", (7, 3)),
    ("FILTER_FLAG_SCHEME_REQUIRED", (7, 3)),
    // Magic-quotes sanitiser: deprecated 7.4, removed 8.0 (stub @deprecated 7.4).
    ("FILTER_SANITIZE_MAGIC_QUOTES", (7, 4)),
    // FILTER_SANITIZE_STRING: deprecated 8.1 (RFC), still present. Stub
    // @deprecated 8.1.
    ("FILTER_SANITIZE_STRING", (8, 1)),
];

/// Editorial constant deprecation successors. Empty: none of the deprecated
/// constants above has a single canonical successor the PHP manual endorses
/// (`E_STRICT` and the removed filter flags have none; the manual lists no
/// direct replacement for `FILTER_SANITIZE_STRING`). The slot exists and is
/// guarded exactly like the function replacements.
pub(crate) const CONSTANT_REPLACEMENTS: &[(&str, &str)] = &[];

// ---------------------------------------------------------------------------
// Class tables (M4). Classes, interfaces and enums collapse into one
// case-INSENSITIVE table keyed by lowercased FQN.
// ---------------------------------------------------------------------------

/// Extensions whose class-likes the diff mis-dates forward because the extension
/// is absent at the 7.4 floor build. `tidy`/`xsl`/`zip` are compiled only in
/// some 8.x builds, so their ancient classes (TidyNode, XSLTProcessor,
/// ZipArchive) read as in-range and are corrected to pre-floor `None`. Reviewed:
/// a correction for an extension not listed here fails generation.
pub(crate) const CLASS_ADDED_ARTIFACT_EXTENSIONS: &[&str] = &["tidy", "xsl", "zip"];

/// Reviewed per-symbol class `added` overrides. Empty: the cache diff agrees with
/// PHPCompatibility's NewClassesSniff for every class (0 disagreements). The slot
/// exists for a future class the diff and artefact rule cannot date. (lowercased)
pub(crate) const CLASS_ADDED_OVERRIDES: &[(&str, Option<(u8, u8)>)] = &[];

/// Reviewed per-symbol class `removed` overrides: the DOM Level 3 classes that
/// phpstorm-stubs drops at 8.0. Nine carry an explicit stub `@removed = 8.0`;
/// `domstringlist` and `domstringextend` disappear from the 8.0 cache in the same
/// wave without an explicit annotation. DOM is compiled in every build, so these
/// are genuine removals, not a whole-extension artefact, and PHPCompatibility's
/// RemovedClassesSniff is silent on them, so they are pinned here. (lowercased)
pub(crate) const CLASS_REMOVED_OVERRIDES: &[(&str, Option<(u8, u8)>)] = &[
    ("domconfiguration", Some((8, 0))),
    ("domdomerror", Some((8, 0))),
    ("domerrorhandler", Some((8, 0))),
    ("domimplementationlist", Some((8, 0))),
    ("domimplementationsource", Some((8, 0))),
    ("domlocator", Some((8, 0))),
    ("domnamelist", Some((8, 0))),
    ("domstringextend", Some((8, 0))),
    ("domstringlist", Some((8, 0))),
    ("domtypeinfo", Some((8, 0))),
    ("domuserdatahandler", Some((8, 0))),
];

/// Extensions whose class-likes vanish wholesale from a late reflection build
/// because the extension was not compiled there (`ftp` at 8.5; `tidy`/`xsl`/`zip`
/// drop after 8.3). A PHPCompatibility-silent disappearance for one of these is a
/// build artefact -> `removed: None`; a silent disappearance outside this
/// allowlist fails generation. DOM is deliberately absent: its removals are real
/// (see [`CLASS_REMOVED_OVERRIDES`]).
pub(crate) const CLASS_REMOVED_ARTIFACT_EXTENSIONS: &[&str] = &["ftp", "tidy", "xsl", "zip"];

/// Editorial class deprecation versions: the sole source of class `deprecated`.
/// The reflection caches carry no usable class `isDeprecated` (it is null for
/// every class) and PHPCompatibility ships no class-deprecation sniff, so this is
/// a reviewed PHP-manual list, fact-locked, like the constant deprecations. Empty
/// for now: no whole-class deprecation in 7.4..8.5 is curated. (lowercased keys)
pub(crate) const CLASS_DEPRECATIONS: &[(&str, (u8, u8))] = &[];

/// Editorial class deprecation successors. Empty (no deprecated classes yet).
pub(crate) const CLASS_REPLACEMENTS: &[(&str, &str)] = &[];
