import json
A="crates/sheaf-app/"; U=A+"ui/"; S=A+"src/"; C="crates/sheaf-core/src/"; L="crates/sheaf-cli/src/"
O=U+"screens/options.slint"; TF=U+"screens/tool-flow.slint"; M=U+"main.slint"
CLI=L+"main.rs"; MCP=L+"mcp.rs"
out=[]
def g(area,feature,behavior,ev,limits=""):
    out.append({"area":area,"feature":feature,"behavior":behavior,"evidence":ev if isinstance(ev,list) else [ev],"limits":limits})

H="Home & files"
g(H,"Open file button on Recents","Click 'Open file' on the Recents page to pick one or more PDFs; they are held and the Tools page opens with a 'Choose a tool for your file' toast.",[U+"screens/recents.slint:55",S+"main.rs:192",S+"flow.rs:378"],"Only PDFs are offered by the dialog; images or Office files cannot be opened from Home.")
g(H,"Drop files on the window (Home)","Drop a PDF anywhere on the window while on Home; it is kept and the Tools page opens so you pick a tool.",[S+"main.rs:283",S+"files.rs:65",S+"flow.rs:364"],"Non-PDFs dropped on Home are refused with 'Open a tool first to add …'.")
g(H,"Drop files on a tool tab","Drop a file on the window while a tool tab is active; it is added to that tool if the extension is accepted.",[S+"files.rs:65",S+"flow.rs:364"],"")
g(H,"Drop rejection toast","Dropping a file a tool does not accept shows 'Not supported by <tool>: <name>'.",[S+"files.rs:48"],"")
g(H,"Folder drop refused","Dropping a folder shows 'Folders cannot be added: <name>' instead of adding its contents.",[S+"files.rs:66"],"No recursive folder import.")
g(H,"Home files carried into the tool","After opening PDFs from Home, clicking a tool opens it preloaded with the held files it accepts.",[S+"main.rs:197",S+"flow.rs:50"],"Held files the tool does not accept are dropped silently.")
g(H,"Open a tool's file picker","Click 'Open file' in an empty tool tab to get a file dialog filtered to that tool's extensions; multi-file tools allow several.",[U+"widgets/drop-zone.slint:12",S+"flow.rs:66"],"")
g(H,"Add more files with + button","Click the round red '+' (with file count badge) in a tool tab to add more files.",[U+"widgets/drop-zone.slint:28",TF+":277"],"")
g(H,"File dialog remembers folder","The add-files dialog opens in the folder of the last file added to the tab.",[S+"flow.rs:70"],"Not remembered across tabs or sessions.")
g(H,"Single-file tools replace the file","On a single-file tool, adding a new file replaces the current one.",[S+"flow.rs:127"],"")
g(H,"Remove a file from the grid","Hover a file card and click × to remove it from the tool.",[U+"widgets/file-card.slint:120",S+"flow.rs:290"],"")
g(H,"Reorder files with arrows","Hover a file card in a multi-file tool and click ← or → to move it one place.",[U+"widgets/file-card.slint:126",S+"flow.rs:304"],"No drag-and-drop reordering of file cards; one step at a time.")
g(H,"File card thumbnail and caption","Each added file shows a page-1 thumbnail with size and page count ('3.17 MB - 7 pages').",[U+"widgets/file-card.slint:76",S+"files.rs:97"],"")
g(H,"Locked badge on file cards","A protected file shows a lock badge on its card.",[U+"widgets/file-card.slint:83"],"")
g(H,"Preview unavailable notice","A file whose preview crashed stays with '⚠ preview unavailable'.",[U+"widgets/file-card.slint:111",S+"files.rs:160"],"")
g(H,"Password prompt on add","Adding a protected PDF opens a '<file> is password protected' dialog; type the password and press Enter or Unlock.",[U+"screens/dialogs.slint:50",S+"password.rs:9"],"")
g(H,"Wrong password shake","A wrong password shakes the dialog, clears the field and shows 'Wrong password'.",[U+"screens/dialogs.slint:61"],"")
g(H,"Skip password","Click Skip in the password dialog to leave that file out.",[U+"screens/dialogs.slint:82",S+"password.rs:82"],"")
g(H,"Password queue","Several protected files queue; the dialog asks for each in turn.",[S+"password.rs:1"],"No 'use this password for all' option.")
g(H,"Unreadable file toast","A PDF that cannot be opened is removed with a 'Cannot open <name>: <error>' toast.",[S+"flow.rs:218"],"")
g(H,"Repair keeps broken files","In Repair, a file that cannot be previewed stays with 'Can't preview — will try to repair'.",[S+"files.rs:160",S+"state.rs:199"],"")
g(H,"Toast messages","Errors, rejections and hints appear as a bottom-centre toast for 3 seconds.",[U+"screens/dialogs.slint:125",S+"flow.rs:27"],"No toast history; messages disappear after 3 s.")
g(H,"Tabs per tool","Each opened tool gets its own tab with icon and name; click a tab to switch, its options survive switching.",[U+"widgets/tab-bar.slint:6",M+":201"],"No keyboard shortcut to switch or close tabs.")
g(H,"Home tab","Click the Home tab to go back to Recents/Tools/History/Preferences.",[U+"widgets/tab-bar.slint:67"],"")
g(H,"New tab (+)","Click + in the tab bar to return to the Tools page and pick another tool.",[U+"widgets/tab-bar.slint:101",M+":150"],"")
g(H,"Close a tab","Click × on a tab to close it.",[U+"widgets/tab-bar.slint:42",S+"flow.rs:351"],"No Cmd+W.")
g(H,"Close tab with running job","Closing a tab with a running job asks 'A task is still running' with Keep or Cancel job.",[U+"screens/dialogs.slint:95",S+"main.rs:206"],"")
g(H,"Background job finishes after close","If the tab was closed while a job ran on, the result is still recorded and a '<tool> finished: <file>' toast appears.",[S+"run.rs:382"],"")
g(H,"Sidebar navigation","Click Recents, Tools, Tasks History or Preferences in the left sidebar.",[M+":19"],"")
g(H,"Sidebar collapses on narrow windows","Below 1000 px wide the sidebar shows icons only.",[M+":164",U+"theme.slint:103"],"")
g(H,"Recent files grid","Recents lists up to 12 recent output files with thumbnail, size, pages and age.",[U+"screens/recents.slint:70",S+"home.rs:20"],"Recents are outputs of Sheaf jobs only, not files merely opened.")
g(H,"Open a recent file","Click a recent card to open the file in the default app.",[U+"widgets/recent-card.slint:63",S+"home.rs:256"],"Opens externally; Sheaf has no reader view of its own.")
g(H,"Reveal a recent file","Hover a recent card and click the folder icon to show it in Finder/Explorer.",[U+"widgets/recent-card.slint:74",S+"home.rs:262"],"")
g(H,"Recents empty state","With no recents, an illustration and 'Your recent files will appear here' show.",[U+"screens/recents.slint:93"],"")
g(H,"Missing recents dropped","Recents whose file no longer exists are hidden on refresh.",[S+"home.rs:180"],"")
g(H,"Window size","Window opens at 1280×800 with a 900×600 minimum.",[M+":138"],"")

T="Tools grid"
g(T,"Tools grid by category","The Tools page shows 28 tool cards under six collapsible categories.",[U+"screens/tools.slint:81",C+"tools.rs:147"],"")
g(T,"Collapse a category","Click a category heading to collapse or expand its tools.",[U+"screens/tools.slint:105"],"Collapse state is not saved between launches.")
g(T,"Sort: Categories or All","Use the Sort dropdown to switch between grouped categories and one flat grid.",[U+"screens/tools.slint:61",U+"widgets/sort-combo.slint:4"],"No alphabetical sort, no favourites.")
g(T,"Open a tool","Click a tool card to open it in a new tab.",[U+"widgets/tool-card.slint:9",S+"flow.rs:50"],"")
g(T,"Tool search","(Not implemented) there is no search box to filter tools.",[U+"screens/tools.slint:28"],"No tool search field.")
g(T,"Tile colour and icon","Each tool has a coloured tile with a Lucide icon, reused on its tab.",[S+"icons.rs:1",S+"main.rs:66"],"")

OG="Organize pages"
g(OG,"Merge PDFs","Add two or more PDFs and click Merge PDF to combine them in the shown order.",[C+"pages.rs:58",TF+":143"],"Needs at least 2 files; no per-file page ranges.")
g(OG,"Reorder files for merge","Hover a file and use ← / → to set the merge order.",[U+"widgets/file-card.slint:126"],"No drag reordering.")
g(OG,"Name the merged file","Type an 'Output file name'; .pdf is added if missing.",[O+":280",S+"dest.rs:36"],"")
g(OG,"Split by range","Choose 'Split by range' and type ranges like 1-3,5,8-10 to get one file per range.",[O+":289",C+"pages.rs:74"],"")
g(OG,"Split every N pages","Choose 'Split every N pages' and set pages per file with the −/+ stepper.",[O+":295",O+":13",C+"pages.rs:85"],"Stepper has no typed entry; no split by size or bookmarks.")
g(OG,"Split output folder","Split asks for a folder and writes the parts there.",[S+"dest.rs:22",S+"dest.rs:77"],"")
g(OG,"Remove pages","Type pages to remove ('1-3,5') and run Remove pages.",[O+":313",C+"pages.rs:93"],"Pages are typed, not picked visually.")
g(OG,"Extract pages","Type pages to extract and run Extract pages to get a new PDF.",[O+":313",C+"pages.rs:65"],"One output only; no 'each page a separate file' option.")
g(OG,"Page range syntax help","An info box explains '2,4-6 means pages 2, 4, 5 and 6'.",[O+":320"],"")
g(OG,"Organize page grid","Open Organize to see every page as a thumbnail card.",[TF+":260",S+"flow.rs:244"],"Single PDF only; cannot combine pages from several files.")
g(OG,"Reorder pages with arrows in Organize","Hover a page and click ← / → to move it one place.",[U+"widgets/file-card.slint:187",S+"main.rs:229"],"No drag-and-drop of thumbnails despite the catalogue description; no multi-select.")
g(OG,"Delete a page in Organize","Hover a page and click × to drop it.",[U+"widgets/file-card.slint:181",S+"main.rs:224"],"")
g(OG,"Insert blank page","Click 'Insert blank page' to append a blank page at the end.",[O+":323",S+"main.rs:235"],"Blank page is always added at the end, then moved with arrows.")
g(OG,"Duplicate a page","(Not implemented) the catalogue says 'duplicate' but there is no duplicate control.",[C+"tools.rs:152",U+"widgets/file-card.slint:147"],"No duplicate action in the UI; only CLI --order can repeat a page.")
g(OG,"Rotate one page in Organize","(Not implemented) Organize has no per-page rotate.",[U+"widgets/file-card.slint:147"],"Per-page rotation only through Rotate's page field.")
g(OG,"Preview failure warning","If page thumbnails fail, '⚠ Page previews unavailable' shows.",[TF+":270"],"")
g(OG,"Rotate pages","Choose 90°, 180° or 270° clockwise and run Rotate PDF on one or more PDFs.",[O+":332",C+"rotate.rs:6"],"No counter-clockwise button (CLI only via negative degrees); no visual preview.")
g(OG,"Rotate selected pages only","Type pages in 'Pages (optional)' to rotate only those.",[O+":343"],"")
g(OG,"Batch rotate","Add several PDFs to Rotate to process them all at once.",[C+"tools.rs:153",S+"run.rs:180"],"")

OP="Optimize"
g(OP,"Choose compression level","Pick Extreme, Recommended or Low compression and run Compress.",[O+":349",C+"compress.rs:8"],"Only image recompression; fonts and vectors untouched.")
g(OP,"Extreme compression","Extreme downsizes images to 1000 px long side at JPEG quality 50.",[C+"compress.rs:13"],"")
g(OP,"Recommended compression","Recommended downsizes to 1600 px at quality 70 (default).",[C+"compress.rs:13"],"")
g(OP,"Low compression","Low downsizes to 2400 px at quality 85.",[C+"compress.rs:13"],"")
g(OP,"Batch compress","Add several PDFs to compress them all.",[C+"tools.rs:154"],"")
g(OP,"Never larger guarantee","If compressing does not help, the original bytes are copied and the result says it is already as small as it gets.",[C+"compress.rs:29",S+"state.rs:360"],"")
g(OP,"Compression savings shown","The result shows size before → after and '% smaller!'.",[S+"state.rs:353",U+"screens/result.slint:329"],"No target size or custom DPI/quality.")
g(OP,"Protected input warning (compress)","Compressing a protected PDF warns the output is not password-protected, and says if the copy grew.",[S+"state.rs:233",S+"state.rs:239"],"")
g(OP,"Repair PDF","Add damaged PDFs and run Repair to rebuild them.",[O+":400",C+"repair.rs:1"],"")
g(OP,"OCR PDF","Run OCR to add an invisible searchable text layer to scanned pages.",[O+":598",C+"ocr.rs:1"],"")
g(OP,"Choose OCR language","Tick document languages in the OCR panel.",[O+":600",S+"main.rs:263"],"Only English is listed (pdfcraft LANGUAGES = [en]); the job uses only the first ticked language; Latin script only.")
g(OP,"Also read pages with text","Tick 'Also read pages with text' to OCR pages that already have text, adding only new words.",[O+":607",C+"ocr.rs:1"],"")
g(OP,"OCR models missing notice","If the models are missing, the panel warns and the button stays disabled.",[O+":611",O+":195"],"")
g(OP,"OCR result summary","The result reports pages read, words added and pages that already had text.",[C+"tools.rs:226"],"No deskew, no output as image-only PDF.")

CT="Convert to PDF"
g(CT,"Images to PDF","Add JPG, PNG, WebP, TIFF or BMP images and run JPG to PDF to get one PDF.",[C+"tools.rs:157",C+"images.rs:62"],"One combined PDF only; no one-PDF-per-image option.")
g(CT,"Image order","Use ← / → on image cards to set page order.",[U+"widgets/file-card.slint:126"],"")
g(CT,"Page size: Fit to image","Choose 'Fit to image' so each page matches its image.",[O+":407",S+"run.rs:36"],"")
g(CT,"Page size: A4 portrait","Choose 'A4 portrait' to centre each image on an A4 page.",[O+":408"],"No Letter or other paper sizes.")
g(CT,"Page size: A4 landscape","Choose 'A4 landscape'.",[O+":409"],"")
g(CT,"Image margin","For A4, pick margin None, Small (18 pt) or Big (36 pt).",[O+":413",S+"run.rs:37"],"")
g(CT,"Word to PDF","Add .doc/.docx/.odt/.rtf and convert to PDF through LibreOffice.",[C+"tools.rs:158",C+"office.rs:1"],"Requires a separately installed LibreOffice.")
g(CT,"PowerPoint to PDF","Add .ppt/.pptx/.odp and convert through LibreOffice.",[C+"tools.rs:159"],"Requires LibreOffice.")
g(CT,"Excel to PDF","Add .xls/.xlsx/.ods/.csv and convert through LibreOffice.",[C+"tools.rs:160"],"Requires LibreOffice; no sheet or print-area choice.")
g(CT,"HTML to PDF","Add .html/.htm files and render them as PDF through LibreOffice.",[C+"tools.rs:161"],"Local files only, no URL input; requires LibreOffice.")
g(CT,"LibreOffice missing prompt","Without LibreOffice the panel says to install it and offers 'Get LibreOffice' opening the download page.",[O+":742",S+"main.rs:277"],"")
g(CT,"Offline conversion note","Info box: 'Converted with LibreOffice on this computer. Nothing is uploaded.'",[O+":744"],"")
g(CT,"Conversion timeout","Each Office conversion times out after 120 s.",[C+"office.rs:15"],"")
g(CT,"Batch office conversion","Several documents convert in one run; failed ones are listed on the result page.",[S+"run.rs:180",S+"state.rs:381"],"")

CF="Convert from PDF"
g(CF,"PDF to JPG","Export every page as JPEG into a chosen folder.",[C+"images.rs:11",O+":423"],"JPEG only (no PNG/TIFF output in the app); all pages, no range.")
g(CF,"JPG quality: Normal","Normal quality = 150 dpi, JPEG 85.",[O+":426",S+"run.rs:226"],"")
g(CF,"JPG quality: High","High quality = 300 dpi, JPEG 92.",[O+":432"],"")
g(CF,"PDF to Word","Export a PDF to an editable .docx.",[C+"export.rs:1"],"Layout is approximate.")
g(CF,"PDF to PowerPoint","Export a PDF to .pptx, one slide per page.",[C+"export.rs:1",CLI+":268"],"Layout is approximate.")
g(CF,"PDF to Excel","Recover tables from a PDF into .xlsx.",[C+"excel.rs:1"],"Table detection is heuristic; text outside tables goes in column A.")
g(CF,"Excel layout: one sheet","Choose 'One sheet' to put all tables on one sheet.",[O+":645"],"")
g(CF,"Excel layout: multiple sheets","Choose 'Multiple sheets' for one sheet per table.",[O+":646"],"")
g(CF,"OCR for scanned exports","Choose 'OCR (scanned PDFs)' to read scanned pages before export to Word/PowerPoint/Excel.",[O+":625"],"Same English/Latin-only OCR.")
g(CF,"No OCR for text PDFs","Choose 'No OCR (text PDFs)' (default).",[O+":619"],"")
g(CF,"Approximate layout warning","Info box warns that exact formatting may differ.",[O+":650"],"")
g(CF,"PDF to PDF/A","Convert to the archival PDF/A format and check the result.",[C+"pdfa.rs:1"],"")
g(CF,"PDF/A level 2b","Choose PDF/A-2b (recommended).",[O+":678"],"No PDF/A-1b or -a (accessible) levels.")
g(CF,"PDF/A level 3b","Choose PDF/A-3b (allows attachments).",[O+":684"],"")
g(CF,"PDF/A preflight report","The result lists rules not fixed, with ISO 19005 clause.",[S+"state.rs:258",C+"pdfa.rs:84"],"")

E="Edit"
g(E,"Add text mode","Choose 'Add text' and click on a page to place a text box.",[U+"screens/edit-panel.slint:67",S+"edit_ui.rs:125"],"Only adds new text; cannot edit existing PDF text.")
g(E,"Type text for selected box","Type the selected box's text in the panel field; it updates on the page.",[U+"screens/edit-panel.slint:83",S+"edit_ui.rs:281"],"Single-line field in the app (CLI supports \\n).")
g(E,"Text size −/+","Step text size through 4–200 pt with − / +.",[U+"screens/edit-panel.slint:88",S+"edit_ui.rs:16"],"Helvetica only; no font choice, bold or italic.")
g(E,"Text colour","Pick black, grey, red or blue swatch.",[U+"screens/edit-panel.slint:114",S+"edit_ui.rs:20"],"Four colours only; no picker.")
g(E,"Add image mode","Choose 'Add image', pick a PNG/JPEG, then click a page to place it centred.",[U+"screens/edit-panel.slint:73",S+"edit_ui.rs:329"],"PNG/JPEG only.")
g(E,"Image size","Choose Small (100 pt), Medium (150 pt) or Large (250 pt) image width.",[U+"screens/edit-panel.slint:138",S+"edit_ui.rs:22"],"No free resize handles.")
g(E,"Select an item","Click a placed text or image to select it.",[S+"edit_ui.rs:119"],"")
g(E,"Move an item by drag","Drag a placed item to move it; it stays on its page.",[S+"edit_ui.rs:125",S+"viewer_ui.rs:316"],"Cannot drag an item to another page.")
g(E,"Remove an item","Click 'Remove text' / 'Remove image' to delete the selected item.",[U+"screens/edit-panel.slint:150",S+"edit_ui.rs:320"],"No undo.")
g(E,"Edit readiness message","Panel shows item count and why it cannot run (empty box, undrawable character).",[U+"screens/edit-panel.slint:155",S+"edit_ui.rs:162",C+"edit.rs:153"],"Non-Latin characters refused (Helvetica).")
g(E,"Edit annotations / shapes","(Not implemented) no highlight, shapes, comments, whiteout or editing existing content.",[C+"edit.rs:1"],"Edit: only text and images, no annotations.")
g(E,"Page numbers","Stamp page numbers on one or more PDFs.",[C+"numbers.rs:1",O+":439"],"")
g(E,"Page numbers: single or double-sided","Choose Single page or Double-sided (mirrored on even pages).",[O+":444"],"")
g(E,"Page numbers: cover page","Tick 'First page is cover page' to skip numbering it.",[O+":447"],"")
g(E,"Page numbers: position","Pick one of six top/bottom positions on the page grid.",[O+":452",O+":82"],"")
g(E,"Page numbers: margin","Choose Small, Recommended or Big margin.",[O+":458"],"")
g(E,"Page numbers: page range","Type From page / To page to number only part of a single PDF.",[O+":464"],"Ignored with several PDFs (all pages numbered).")
g(E,"Page numbers: first number","Type the first number shown.",[O+":466"],"")
g(E,"Page numbers: text format","Choose number only, 'Page {n}', 'Page {n} of {p}' or Custom.",[O+":468"],"No font, size or colour choice in the app (CLI has --font-size).")
g(E,"Page numbers: custom text","Type a custom template with {n} and {p}; errors are validated live.",[O+":473",S+"main.rs:259"],"")
g(E,"Watermark text","Choose 'Place Text', type the text and font size.",[O+":484",O+":489"],"Font fixed; no bold/italic.")
g(E,"Watermark colour","Pick Black, Grey, Red or Blue.",[O+":491",S+"run.rs:119"],"Four colours (CLI accepts any RRGGBB).")
g(E,"Watermark image","Choose 'Place Image' and pick a PNG/JPEG.",[O+":498",S+"main.rs:268"],"")
g(E,"Watermark image size","Pick 25, 50, 75 or 100%.",[O+":515"],"")
g(E,"Watermark position","Pick one of nine positions on the grid.",[O+":528"],"")
g(E,"Watermark layer","Choose Over the content or Below the content.",[O+":534"],"")
g(E,"Watermark mosaic","Tick Mosaic to tile the mark across the page.",[O+":537",C+"watermark.rs:93"],"")
g(E,"Watermark transparency","Choose No, 25%, 50% or 75% transparency.",[O+":540"],"")
g(E,"Watermark rotation","Choose None, 45°, 90°, 180°, 270° or −45°.",[O+":548"],"")
g(E,"Watermark page range","Type From/To page on a single PDF.",[O+":561"],"Ignored for several PDFs; no live preview of the watermark.")
g(E,"Crop all pages","Choose 'All pages' and type margins to cut in mm.",[O+":568",C+"crop.rs:37"],"")
g(E,"Crop selected pages","Choose 'Selected pages' and type which pages.",[O+":569"],"")
g(E,"Remove crop","Choose 'Remove crop' to show full pages again.",[O+":570"],"")
g(E,"Crop margins in mm","Type top, bottom, left, right margins (decimal comma accepted).",[O+":574",S+"run.rs:55"],"No drag-a-box crop on the page.")
g(E,"Crop live preview","Page 1 previews with cut margins dimmed and kept area outlined.",[O+":585",O+":123"],"Preview shows page 1 only.")

SG="Sign"
g(SG,"Type a signature","Choose Type and enter your full name; it is drawn in a script font.",[U+"screens/sign-panel.slint:115",S+"sign_ui.rs:89"],"One script font; character limit MAX_SIGNATURE_CHARS.")
g(SG,"Draw a signature","Choose Draw and draw with the mouse on the pad.",[U+"screens/sign-panel.slint:17",S+"sign_ui.rs:95"],"No pen colour or thickness choice.")
g(SG,"Clear drawn signature","Click Clear under the pad.",[U+"screens/sign-panel.slint:127"],"")
g(SG,"Upload signature image","Choose Upload and pick a PNG/JPEG signature; bad images are refused at once.",[U+"screens/sign-panel.slint:141",S+"sign_ui.rs:104"],"No background removal.")
g(SG,"Signature preview","The panel previews the typed or uploaded signature.",[U+"screens/sign-panel.slint:160"],"")
g(SG,"Signature size","Choose Small (100 pt), Medium (150 pt) or Large (220 pt).",[U+"screens/sign-panel.slint:182",S+"sign_ui.rs:15"],"No free resize.")
g(SG,"Place signature by click","Click on a page to place the signature centred there.",[S+"viewer_ui.rs:316"],"One signature per run; cannot place on several pages.")
g(SG,"Move signature","Click elsewhere or drag to move it.",[U+"screens/sign-panel.slint:84",S+"viewer_ui.rs:331"],"")
g(SG,"Placed-on-page indicator","Panel shows 'Placed on page N'.",[U+"screens/sign-panel.slint:193"],"")
g(SG,"Digital signature with .p12","Tick 'Use a certificate (.p12)', choose the file and password to sign digitally.",[O+":692",S+"sign_ui.rs:115",C+"sign.rs:1"],"No timestamp server, no signature validation view.")
g(SG,"Signature reason and location","Type optional Reason and Location for the certificate signature.",[O+":730"],"")
g(SG,"Protected PDFs refused","A protected PDF added to Sign is refused: 'Unlock the PDF first, then sign it'.",[S+"files.rs:136"],"")
g(SG,"Request signatures / initials / date fields","(Not implemented) no initials, date or form fill.",[C+"sign.rs:1"],"No fill-in of form fields, no request-signature flow.")

SE="Security"
g(SE,"Protect with password","Type and repeat a password and run Protect (AES-256).",[O+":373",C+"security.rs:6"],"")
g(SE,"Owner password","Optionally type a separate owner password.",[O+":389"],"Permissions are always 'all'; no print/copy restrictions.")
g(SE,"Password match check","Button stays disabled until both passwords match.",[O+":190"],"No strength meter or show-password toggle.")
g(SE,"Batch protect","Protect several PDFs with the same password.",[C+"tools.rs:172"],"")
g(SE,"Unlock PDF","Add protected PDFs, type each password when asked, and save unprotected copies.",[O+":396",C+"security.rs:29"],"Only for PDFs you know the password of.")
g(SE,"Unprotected-output warning","Tools that drop protection say 'The output is not password-protected.'",[S+"state.rs:234"],"")

R="Redact"
g(R,"Search text to redact","Type text and press Enter or Search to find all matches (case-insensitive).",[U+"screens/redact-panel.slint:74",S+"viewer_ui.rs:393"],"No regex or whole-word option.")
g(R,"Redact patterns","Toggle Email, Phone number, Credit card, Date and SSN chips to search for them.",[U+"screens/redact-panel.slint:83",S+"viewer_ui.rs:372"],"Fixed set; no IBAN or custom patterns.")
g(R,"Search results list","Matches list with checkbox, text and page; all checked by default.",[U+"screens/redact-panel.slint:104",S+"viewer_ui.rs:423"],"")
g(R,"Untick a match","Click a match's checkbox to keep it out of redaction.",[U+"screens/redact-panel.slint:133",S+"viewer_ui.rs:441"],"")
g(R,"Jump to a match","Click a result row to scroll the viewer to that match.",[U+"screens/redact-panel.slint:122",S+"viewer_ui.rs:449"],"")
g(R,"Drag to mark an area","Drag on a page to mark a rectangle for redaction.",[U+"widgets/page-viewer.slint:200",S+"viewer_ui.rs:339"],"")
g(R,"Mark whole page","Click 'Mark whole page N' to redact the current page.",[U+"screens/redact-panel.slint:166",S+"viewer_ui.rs:459"],"Only the current page; no page range in the app (CLI has --pages).")
g(R,"Marked for redaction list","All marks list with label and page.",[U+"screens/redact-panel.slint:175"],"")
g(R,"Remove a mark","Click × on a marked item.",[U+"screens/redact-panel.slint:240",S+"viewer_ui.rs:479"],"")
g(R,"Clear all marks","Click 'Clear all'.",[U+"screens/redact-panel.slint:181",S+"viewer_ui.rs:493"],"No undo.")
g(R,"Verified redaction","After writing, Sheaf checks no searched text remains and reports it verified (also metadata, bookmarks, comments).",[C+"redact.rs:55",C+"redact.rs:2049",S+"state.rs:372"],"")
g(R,"Redaction leftover notes","Notes list places where searched text remains (links, field names, fonts).",[C+"redact.rs:1088"],"")
g(R,"Redaction review reminder","Info box reminds to review results before sending.",[U+"screens/redact-panel.slint:251"],"No preview of the redacted output before saving; box colour fixed black.")

CP="Compare"
g(CP,"Original and Changed slots","Open a file into the Original slot and one into the Changed slot.",[TF+":52",S+"flow.rs:85"],"")
g(CP,"Drop into compare slots","Dropped or multi-picked files fill empty slots in order.",[S+"flow.rs:115",S+"state.rs:125"],"")
g(CP,"Replace a compare file","Click 'Replace…' under a filled slot.",[TF+":102"],"")
g(CP,"Swap original and changed","Click Swap between the slots.",[TF+":183",S+"flow.rs:97"],"")
g(CP,"Semantic Text compare","Choose 'Semantic Text' to get a report PDF of word changes.",[O+":656",C+"compare.rs:1"],"Report is a new PDF; no in-app side-by-side view.")
g(CP,"Content Overlay compare","Choose 'Content Overlay' to get the changed PDF with changes highlighted.",[O+":662",C+"compare.rs:1"],"")
g(CP,"Compare summary","Result counts added, removed, changed words and changed areas.",[C+"compare.rs:52"],"No synchronized scrolling viewer.")

V="Viewer"
g(V,"Continuous page scroll","In Redact, Sign and Edit, all pages show in a vertical scroll.",[U+"widgets/page-viewer.slint:130",S+"viewer.rs:85"],"Viewer only exists inside Redact, Sign and Edit; no standalone reader.")
g(V,"Thumbnail strip","A left strip shows page thumbnails; the current page is outlined.",[U+"widgets/page-viewer.slint:89",U+"widgets/page-viewer.slint:104"],"")
g(V,"Click thumbnail to jump","Click a thumbnail to scroll to that page.",[U+"widgets/page-viewer.slint:116",S+"viewer_ui.rs:264"],"")
g(V,"Zoom in button","Click + to step zoom (50,67,80,100,125,150,200%).",[U+"widgets/page-viewer.slint:267",S+"viewer.rs:119"],"Max 200%.")
g(V,"Zoom out button","Click − to step zoom down.",[U+"widgets/page-viewer.slint:258"],"Min 50%.")
g(V,"Fit width button","Click Fit to return to 100% (fit width).",[U+"widgets/page-viewer.slint:268",S+"viewer_ui.rs:271"],"No fit-page mode.")
g(V,"Zoom in the page viewer with Cmd+","Press Cmd+ (or Cmd=) to zoom in.",[U+"widgets/page-viewer.slint:283"],"Only after clicking into the viewer (focus).")
g(V,"Zoom out with Cmd−","Press Cmd− to zoom out.",[U+"widgets/page-viewer.slint:287"],"")
g(V,"Reset zoom with Cmd0","Press Cmd0 to fit width.",[U+"widgets/page-viewer.slint:291"],"")
g(V,"Zoom keeps place","Zooming keeps the same part of the document in view.",[S+"viewer_ui.rs:279"],"No pinch or Ctrl+scroll zoom.")
g(V,"Zoom percentage label","Current zoom shows between − and +.",[U+"widgets/page-viewer.slint:261"],"")
g(V,"Lazy page rendering","Only pages near the viewport render; cache limited to 96 MB.",[S+"viewer.rs:226",S+"viewer_ui.rs:184"],"")
g(V,"Viewer status","'Opening…' or an error shows while loading.",[U+"widgets/page-viewer.slint:236",S+"state.rs:324"],"")
g(V,"Outline / search / page nav","(Not implemented) no outline/TOC panel, no find-in-document, no page number entry.",[U+"widgets/page-viewer.slint:29"],"Viewer has no outline/TOC panel, no Ctrl+F, no go-to-page, no keyboard page navigation.")

RS="Results & saving"
g(RS,"Ask where to save","With 'Ask every time', a Save dialog with the default name opens for one output.",[S+"dest.rs:77"],"")
g(RS,"Choose folder for many outputs","Multiple outputs (Split, PDF to JPG, batches) ask for a folder.",[S+"dest.rs:45",S+"dest.rs:80"],"")
g(RS,"Default output names","Outputs get suffixes like -merged, -compressed, -signed, -redacted.",[S+"jobs.rs:495"],"")
g(RS,"Never overwrite","An existing name gets ' (1)' instead of being overwritten.",[C+"paths.rs:15",C+"paths.rs:43"],"")
g(RS,"Cancel save dialog","Cancelling the Save dialog aborts the run silently.",[S+"run.rs:322"],"")
g(RS,"Run button enabled only when ready","The action button enables only when options are valid and files are ready.",[TF+":362",O+":189"],"")
g(RS,"Progress overlay","While running, a progress bar with percentage covers the file area.",[U+"screens/processing.slint:4",S+"run.rs:336"],"")
g(RS,"Cancel a job","Click Cancel; 'Cancelling…' shows and written outputs are removed.",[U+"screens/processing.slint:62",S+"run.rs:391",S+"jobs.rs:148"],"")
g(RS,"Result page","After a run, the tab shows output cards, a summary bar and 'Continue to…'.",[U+"screens/result.slint:41"],"")
g(RS,"Open an output","Click the file icon on an output card to open it.",[U+"screens/result.slint:195",S+"run.rs:444"],"")
g(RS,"Reveal an output","Click the folder icon on an output card to show it in Finder.",[U+"screens/result.slint:199",S+"run.rs:450"],"")
g(RS,"Open folder","Click 'Open folder' to reveal the first output.",[U+"screens/result.slint:361",S+"run.rs:456"],"")
g(RS,"Start over","Click the start-over control to clear files and run the same tool again.",[U+"screens/result.slint:356",S+"run.rs:462"],"")
g(RS,"Continue to another tool","Click a tool under 'Continue to…' to open it with these outputs.",[U+"screens/result.slint:210",S+"run.rs:479",S+"state.rs:270"],"")
g(RS,"Failure list","Files that failed in a batch are listed with their error.",[U+"screens/result.slint:73",S+"state.rs:381"],"")
g(RS,"Notes list","Tool notes (PDF/A issues, redaction notes) show on the result page.",[U+"screens/result.slint:93",S+"state.rs:375"],"")
g(RS,"Job failure toast","A failed job shows '<tool> failed: <error>' and the tab returns to editing.",[S+"run.rs:374"],"")
g(RS,"Output fixed folder missing","If the fixed folder no longer exists, a toast says so and the Save dialog opens.",[S+"run.rs:316"],"")
g(RS,"Print / share / email","(Not implemented) no print or share actions.",[U+"screens/result.slint:41"],"No print, share sheet or drag-out of results.")

HR="History & recents"
g(HR,"Tasks History list","Tasks History lists every finished job (tool, files, when), newest first.",[U+"screens/history.slint:94",S+"home.rs:128"],"Capped at 200 entries.")
g(HR,"Reveal a history output","Click the folder icon on a row to show the output (disabled if missing).",[U+"screens/history.slint:55",S+"home.rs:268"],"")
g(HR,"Clear history","Click 'Clear all' then confirm 'Clear the whole history?'.",[U+"screens/history.slint:116",U+"screens/history.slint:65",S+"home.rs:149"],"")
g(HR,"Relative times","Times show relatively and refresh when a Home page is shown.",[S+"history.rs:84",M+":133"],"")
g(HR,"Rerun from history","(Not implemented) history rows cannot be re-run or filtered.",[U+"screens/history.slint:8"],"No search, filter or re-run.")

PF="Preferences"
g(PF,"Appearance","Choose System, Light or Dark.",[U+"screens/preferences.slint:116",S+"home.rs:100"],"")
g(PF,"Theme env override","SHEAF_THEME=dark|light forces appearance.",[S+"main.rs:163"],"Debug only.")
g(PF,"Output: ask every time","Choose 'Ask every time' for a Save dialog each run.",[U+"screens/preferences.slint:131"],"")
g(PF,"Output: next to original","Choose 'Save next to the original file'.",[U+"screens/preferences.slint:136",S+"dest.rs:51"],"")
g(PF,"Output: fixed folder","Choose 'Always save to a folder' and pick it with Choose….",[U+"screens/preferences.slint:143",S+"home.rs:113"],"")
g(PF,"Open output folder when done","Toggle the switch to reveal outputs after each job.",[U+"screens/preferences.slint:166",S+"home.rs:143"],"")
g(PF,"Preferences saved","Preferences persist in prefs.json.",[S+"prefs.rs:77"],"No language, default compression or naming preferences.")

CL="CLI"
for name,line,desc,lim in [
 ("merge",51,"sheaf merge a.pdf b.pdf -o out.pdf merges in order.",""),
 ("split --ranges",59,"sheaf split --ranges 1-3,5 -o dir/ writes one file per range.",""),
 ("split --every",65,"sheaf split --every N -o dir/ splits every N pages.",""),
 ("extract",71,"sheaf extract --ranges … -o out.pdf.",""),
 ("remove",79,"sheaf remove --pages … -o out.pdf.",""),
 ("organize --order",87,"sheaf organize --order 3,0,1 reorders, 0 inserts a blank page, repeats duplicate.",""),
 ("rotate",95,"sheaf rotate --degrees 90|180|270|-90 [--pages] rotates.",""),
 ("compress --level",106,"sheaf compress --level low|recommended|extreme.",""),
 ("protect",114,"sheaf protect --user-password [--owner-password].",""),
 ("unlock",124,"sheaf unlock with global --password.",""),
 ("repair",130,"sheaf repair rebuilds a damaged PDF.",""),
 ("to-jpg",136,"sheaf to-jpg --dpi --quality 1-100 -o dir/.",""),
 ("from-images",146,"sheaf from-images [--a4 [--landscape]] builds a PDF from images.","Fixed 0.5 in margin with --a4."),
 ("number",159,"sheaf number with --position, --double-sided, --cover, --margin, --from/--to, --start, --text, --custom, --font-size.",""),
 ("watermark",192,"sheaf watermark --text|--image with font size/auto, RRGGBB colour, scale, position/mosaic, transparency, rotate, --below, range.",""),
 ("crop",228,"sheaf crop --top/--bottom/--left/--right mm, --pages, or --remove.",""),
 ("ocr",248,"sheaf ocr [--language en] [--all-pages].",""),
 ("to-word",260,"sheaf to-word [--ocr].",""),
 ("to-powerpoint",269,"sheaf to-powerpoint [--ocr].",""),
 ("to-excel",278,"sheaf to-excel --layout one-sheet|sheet-per-table [--ocr].",""),
 ("compare",289,"sheaf compare old new --mode text|overlay.",""),
 ("redact",300,"sheaf redact --text (repeatable) --pattern email|phone|credit-card|ssn|date --pages.","No rectangle areas by coordinates."),
 ("sign",316,"sheaf sign --page --at X,Y --width with --text|--image|--strokes JSON, optional --p12 --p12-password --reason --location.",""),
 ("edit",354,"sheaf edit with repeated --text/--image items each with --page --at --size --color --width.",""),
 ("pdfa",385,"sheaf pdfa --level 2b|3b; exit 1 if not conforming.",""),
 ("office",393,"sheaf office converts Office/HTML via LibreOffice.",""),
 ("info",399,"sheaf info prints pages, sizes, encryption, metadata, forms, bookmarks as JSON.",""),
 ("text",403,"sheaf text [--pages] prints page text separated by form feeds.",""),
 ("render",410,"sheaf render --page --dpi 18-600 -o page.png.",""),
 ("tools",422,"sheaf tools prints every command as JSON schema.",""),
]:
    g(CL,"CLI "+name,desc,CLI+":"+str(line),lim)
g(CL,"--password global flag","--password opens every protected input.",CLI+":38","One password for all inputs.")
g(CL,"--json output","--json prints {ok, outputs, notes, data} or {ok:false,error,code}.",[CLI+":41",L+"agent.rs:1"],"")
g(CL,"Exit code 2 on password","Exit code 2 when a password is missing or wrong.",[L+"agent.rs:1","README.md:59"],"")
g(CL,"CLI bundled with app","The CLI ships inside the app bundle (Helpers/sheaf, sheaf-cli.exe, bin/sheaf).",["README.md:46"],"Not added to PATH automatically.")

MC="MCP"
g(MC,"sheaf mcp server","sheaf mcp serves every command as an MCP tool over stdio.",[MCP+":279",CLI+":424"],"stdio only; no resources or prompts.")
g(MC,"MCP tools/list","Tools are generated from the CLI with JSON schemas (tools/mcp/help skipped).",[MCP+":14",MCP+":83"],"")
g(MC,"MCP edit items schema","The edit tool takes an items array of text/image objects.",[MCP+":126"],"")
g(MC,"MCP render returns image","render returns the page PNG as image content so agents can look.",[MCP+":248"],"")
g(MC,"MCP password per call","Each tool accepts a password argument (except from-images and office).",[MCP+":106"],"")
g(MC,"MCP initialize instructions","initialize returns instructions: never overwrites, read outputs, use info/text/render.",[MCP+":258"],"")
g(MC,"Add to Claude Code","README shows 'claude mcp add sheaf -- …/sheaf mcp'.",["README.md:72"],"")

PL="Platform"
g(PL,"Open a file from Finder Open With","Choose Sheaf in Finder's Open With (or drop on Dock icon); PDFs go to Home to pick a tool.",[S+"open_with.rs:112",S+"main.rs:293",A+"macos/Info.plist:7"],"Registered as Alternate viewer; non-PDFs skipped with a toast.")
g(PL,"Open files from command line","Passing file paths to the app opens them like Open With.",[S+"main.rs:297",S+"open_with.rs:79"],"")
g(PL,"Queued launch files","Files arriving before the window is ready are queued and delivered in order.",[S+"open_with.rs:21"],"")
g(PL,"Open With skipped files notice","Non-PDF or missing files show 'Not a PDF' / 'File not found' or a count.",[S+"open_with.rs:58"],"")
g(PL,"macOS / Windows / Linux builds","Native builds for macOS arm64, Windows x64, Linux x64.",["README.md:36"],"macOS ad-hoc signed, not notarized; Windows unsigned; no Intel Mac build.")
g(PL,"Offline, no account","Everything runs locally, no upload or telemetry.",["README.md:13"],"")
g(PL,"Native menu bar / shortcuts","(Not implemented) no app menu commands or global shortcuts (Cmd+O, Cmd+W).",[M+":68"],"Only viewer zoom shortcuts exist.")
g(PL,"Window icon","Window uses the app icon on Windows and Linux.",[M+":137"],"")

json.dump(out,open("dev/board/ours.json","w"),indent=1,ensure_ascii=False)
from collections import Counter
c=Counter(o["area"] for o in out); print(len(out)); [print(k,v) for k,v in c.items()]
