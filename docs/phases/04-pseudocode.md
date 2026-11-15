# Phase 04: Pseudocode (engine behaviour reimplemented in `anyfont-core`)

Dossier: `04-pseudocode/`. These are the decompiled handlers of the font module (`CdrFnt` in the dossier) and
of the Font Manager data sources (`FontDS`, `WPanose*`, `Type1InfoMessageDS`). Addresses are the dossier's
IDA addresses.

The AnyFont implementation is `crates/anyfont-core`. Each section names the Rust function, and every module's
`//!` header repeats its evidence. The UI-side handlers (menus, dialogs) belong to Phase 06.

## 1. sfnt validation (`CFontInformerOTF_STREAM::Initialize` 0x18017F470, `ReadOffsetTable` 0x18017FC90)

`sfnt::SfntFace::open(data, face)` and `.validate()` run these steps:

```text
open(data, face):
  sig = u32(data, 0)
  if sig == 'ttcf':
      n = u32(data, 8); if face >= n -> FaceIndexOutOfRange
      off = u32(data, 12 + 4*face)
  else if sig not in {0x00010000, 'OTTO', 'true'} -> UnknownSignature
  numTables = u16(off + 4); if numTables == 0 -> NoTables
  directory must fit in the file; each table's offset+length must fit -> TableOutOfBounds

validate():
  head present; magic == 0x5F0F3CF5; 16 <= unitsPerEm <= 16384; glyphDataFormat == 0
  maxp present; version 0x00005000 -> CFF flavour, 0x00010000 -> TrueType flavour, else BadMaxpVersion
  cmap present; version < 2
  TrueType flavour: glyf+loca (or bitmap-only EBDT/CBDT/sbix); CFF flavour: 'CFF ' or CFF2
  hhea/hmtx/name missing -> warning only
  family (name ID 1 or 16) in {Libre Franklin, Libre Franklin Italics, Nunito VF Beta} -> KnownProblemFont
```

There is one unit test per rule (`sfnt::tests`). A file that fails validation becomes a record with
`corrupt = Some(reason)`. Unlike the original, which drops it, AnyFont shows it under the Corrupt status
filter.

## 2. Reading a face (`DumpFontInformationFromFontDataForDatabase` 0x18004E530)

`parse::parse_font_file(path)` returns a `ParsedFile`:

```text
bytes = read(path); sha = sha256(bytes)
match signature:
  'wOFF'  -> decode to sfnt, continue as sfnt (format Woff)
  'wOF2'  -> corrupt "WOFF2 files are not supported"
  sfnt    -> for face in 0..face_count: build_sfnt_records(face)
  Type 1  -> parse_type1 (+ .pfm/.afm companions)
  .pfm    -> its .pfb/.pfa if present, else corrupt "Type 1 outline file (.pfb) is missing"
  else    -> corrupt "not a font file"

build_sfnt_records(face):
  validate (§1); ttf_parser::Face::parse
  names: English (3,1,0x409 -> 3,1,any -> 1,0) for IDs 0..19; localised Windows names per language
  OS/2: weight, width, fsType -> Embedding (GetFontEmbeddingPermissions 0x18017CE60), fsSelection,
        panose, ulUnicodeRange, ulCodePageRange, achVendID
  technology: CFF/CFF2 -> OpenType-PostScript; glyf + DSIG -> OpenType-TrueType; glyf -> TrueType
  features: GSUB+GPOS scripts x language systems x features -> (feature, script, language)
  symbol = cmap (3,0); vertical = vhea or vert/vrt2; monospaced = post.isFixedPitch
  fvar present -> one record per named instance:
      subfamily = instance name; wght -> weight; wdth -> width class; ital/slnt -> italic
      axes[i].value = instance coordinate
```

## 3. Filters (`sub_1800DDB60` registration; `0x180113B50`, `0x180108600`, `0x180108A90`, `0x1801090B0`, `0x180113560`, `0x180115360`, `0x1801096E0`, `0x180112320`, `0x1801153B0`, `0x180116770`)

The original builds an SQL `WHERE` clause with one parenthesised `OR` group per filter collection. Groups are
joined with `AND`. `filters::FilterSet::matches` evaluates the same predicate in memory:

```text
matches(font):
  for each group with checked options:
      if no checked option of the group matches the font -> false
  true
```

| Option | Predicate (original SQL → AnyFont) |
|---|---|
| Installed | `DBInstalled=1 AND DBSystem=0` → `installed && !protected` |
| Not installed | `DBInstalled=0` → `!installed` |
| Protected System Fonts | `DBSystem=1` → `protected` |
| Duplicates | `DBDuplicate!=0` → `duplicate != Unique` |
| Embedding n | `DBEmbedding=n` |
| Technology | `DBTech='…'`; Variable Fonts `DBVariable=1` |
| Light / Regular / Bold | weight bucket `IN (100,200,300)` / `(400,500)` / `(600,700,800,900)` |
| Condensed / Normal / Expanded | `DBWidth IN (1,2,3)` / `(4,5,6)` / `(7,8,9)` |
| Style | PANOSE: Decorative `P0=4`; Display `P0=2`; Script `P0=3`; Symbol `P0=5`; Serif `P0=2 AND P1 IN 2..10`; Sans Serif `P0=2 AND P1 IN 11..15`; Monospaced `(P0=2 AND P3=9) OR (P0=3 AND P3=3) OR (P0=4 AND P3=9)`. AnyFont also accepts `post.isFixedPitch` for Monospaced and a symbol cmap for Symbol |
| Character Range | code-page bit (Latin 0/1, Cyrillic 2, Greek 3, Turkish 4, Hebrew 5, Arabic 6, Japanese 17, Chinese Simplified 18, Korean 19/21, Chinese Traditional 20). Unicode-range bits are used when the font has no code pages, and always for Devanagari (bit 15) |
| OpenType group | `DBFeature='t1' OR DBFeature='t2' …` over the group's tags (24 groups) |

Options are offered only when present in the library (`SELECT DISTINCT` probes) →
`filters::available_options`. The four status options are always shown.

## 4. Duplicates (`DBDuplicate`, index `DBDefaultFamilyIndex(DBDefaultFamily, DBDefaultSubFamily, DBTech, DBWeight, DBWidth, DBItalic, DBDocumentID)`)

`duplicates::compute` works as follows:

```text
key(face) = lower(group family), lower(typographic or legacy subfamily), tech, weight, width, italic
for each key with faces from >= 2 different files (sha):
    master = max by (installed, version number, mtime, -id)
    master -> 1 (Master), others -> 2 (Copy)
everything else -> 0 (Unique)
```

The list hides `Copy` unless "Show duplicate fonts" is on (`DBDuplicate<2`). `Catalog::visible_fonts`
evaluates the sets inside the open library, so a folder that holds only a copy still lists it.

## 5. Incremental rescan (database refresh + `FONTFOLDERS` monitoring)

`scan::scan` and `scan::spawn_scan` run this sequence:

```text
set_installed(request.installed)
files = installed paths + walk(enabled folders, recursive?) filtered by font extension; .pfm -> .pfb
to_parse = files whose (path, size, mtime) differ from the files table (all when force)
parallel workers: parse_font_file(path) (panics caught -> corrupt record)
main thread, batches of 64 in one transaction:
    upsert files(path, size, mtime, sha, format, error)
    if no fonts row has this sha: insert its faces (+ names, features, axes, instances)
    if the path previously had another sha and nothing else uses it: delete those faces
    progress(File{done, total, path}); stop early if cancel flag set
if not cancelled:
    delete files rows not in `files`; delete faces whose sha has no file (remote fonts kept)
    recompute duplicates and store the changed DBDuplicate values
progress(Finished(report))
```

## 6. List query (FontDS list data source)

`catalog::Catalog::query` builds the list:

```text
fonts = library faces                       // My fonts: every local face; folder: path under it; collection: members
fonts -= corrupt                           // unless the Corrupt filter is checked
fonts -= duplicate copies                  // unless show_duplicates (evaluated within the library)
fonts = filter(FilterSet) and search (substring of family, full, PostScript, sub-family, localised names)
if search:  sort by (family, weight, width, italic, legacy family); rows flat
elif group: sort by (family, legacy family, weight, width, italic);
            for each family: >1 face -> FamilyHeader{style_count} + indented faces; 1 face -> plain row
else:       same order, flat
if show_recent: Recent header + MRU faces present in the list, first
status = "<library>: N fonts" or "<library> (filtered): N fonts" (filters or search active)
```

Labels come from `format::face_label`:

| Face | Label |
|---|---|
| basic style with a "Regular" sub-family | `Family (Regular, Normal)` |
| italic, bold, bold italic | `Family (Normal-Italic)`, `Family (Bold)`, `Family (Bold-Italic)` |
| any other weight | the family plus the sub-family: `Arial Black`, `Bahnschrift Light Condensed` |

## 7. Command eligibility (`FontDS.InstallEnabled`, `UninstallEnabled`, `DeleteEnabled`, `DownloadEnabled`, `RemoveFolderOrCollectionEnabled`)

`eligibility::evaluate` returns these values:

```text
remove_folder_or_collection = library != MyFonts            // Favorites included (it is emptied)
install   = any selected face: !installed && usable
uninstall = any selected face: installed && !protected
delete    = any selected face: has file && !protected
download  = any selected face: remote && no file
menu_*    = same rules over selected faces + faces of selected family headers
open_file_location = !all_selected && (one face || one family header) selected
add_to_collection  = selection or current font
remove_from_open_collection = library is a collection
remove_from_collection      = outside collections, selection/current font belongs to a collection
```

The unit test `eligibility::tests::toolbar_states_match_capture` rebuilds the 13 captured contexts and
checks every toolbar button against `toolbar-states.json`.

## 8. PANOSE matching (`WPanoseFontArray::GetFontMatchingForMissingFonts` 0x1801A6A40, `WPanoseFontMatching::FillFontListCallback` 0x1801A7840, `PopulateBestPanoseMatchData`)

`panose::find_substitute` runs:

```text
candidates = usable faces, filtered to the missing font's charset when FontFilterType = SameCodePage,
             sorted by (PANOSE distance, name similarity desc, family, id)
1. exception list entry for the missing family -> its substitute family (closest style)   [Exception]
2. permanent substitution                      -> its substitute family                     [Panose]
3. PostScript name equal                       -> that face
4. same family                                 -> closest style (|weight diff| + 1000*italic mismatch)
5. first candidate with a PANOSE distance       -> its family, closest style
6. best name similarity >= 0.5                  -> its family
7. default font of the preferences
```

The original's distance function could not be recovered, so AnyFont uses its own design:

```text
distance(a, b):
  none if either PANOSE is all zero, or family kinds differ (both > 1)
  sum over digits 1..9 with weights [3,6,5,2,1,1,3,1,1]:
      0 on either side -> 0; "No Fit" (1) on one side only -> 9*w; else w*(a-b)^2
```

`type1_replacements` suggests a non-Type-1 face for each Type 1 face. It first looks for the same family after
stripping suffixes such as MT, Std or Pro, choosing the closest style. Otherwise it takes the closest name
with similarity >= 0.75. `TYPE1_MESSAGE` is the `Type1InfoMessageDS` text.

## 9. Import/Export (`FontDS.OnExportGroups` / `HasGroupsForExport`, `OnImportGroups` / `ReadyForImport`)

`exchange::export_xml` and `exchange::import_xml` work as follows:

```text
export: <Export> MonitoredFolders(folder path enabled recursive) + UserGroups(group name; font 8 attributes + key)
import: add missing folders; each group -> Favorites / existing collection (merge) / new collection;
        font -> by key, else SELECT … WHERE DBFamily=? AND DBSubFamily=? AND DBVersion=? AND DBTech=?
                AND DBWeight=? AND DBWidth=? AND DBItalic=? AND DBManufacturer=? (version relaxed if absent)
```

## 10. Glyph browser (glyph docker data source)

`glyphs::enumerate_file` builds the list:

```text
cmap (all Unicode subtables + (3,0)) -> code point -> glyph, code-point order, one entry per code point
after each one: its depth-first GSUB outputs (single, alternate, ligature), encoded or not, no repeats
                within the chain; their ids also go into the entry's `alternates`
then every glyph id not reached (unencoded), except .notdef and glyphs that draw nothing
classification: Unicode script (Common/Inherited -> "Common") x general category
                (Uppercase, Lowercase, Numbers, Punctuation & Separators, Currency, Mathematical Symbols,
                 Marks, Symbols, Other)
```
