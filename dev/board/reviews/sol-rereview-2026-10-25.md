C-annotate-001: DISAGREE f8 a8 p9 — À la largeur minimale de 900 px, plusieurs outils sortent de la barre sans menu de repli : dev/re/evidence/F-002/39-toolbar-900px.png, crates/sheaf-app/ui/screens/reader-toolbar.slint:55
C-annotate-002: AGREE 9
C-annotate-003: AGREE 9
C-annotate-004: AGREE 9
C-annotate-006: AGREE 9
C-annotate-007: DISAGREE f8 a9 p9 — Le texte se saisit dans une fenêtre flottante puis apparaît après « Done », au lieu d’être saisi directement sur la page : crates/sheaf-app/ui/screens/reader.slint:401
C-annotate-009: DISAGREE f8 a9 p9 — Le choix se limite à trois familles ; changer la taille d’une boîte utilisant une autre police remplace aussi sa police : crates/sheaf-app/src/reader_ui/tools.rs:144
C-annotate-010: AGREE 9
C-annotate-011: AGREE 9
C-annotate-012: DISAGREE f8 a9 p9 — Maj ne contraint ni le rectangle en carré ni l’ovale en cercle : crates/sheaf-app/src/reader_ui/edits.rs:210
C-annotate-016: AGREE 9
C-annotate-018: AGREE 9
C-annotate-019: DISAGREE f8 a9 p9 — Le tampon daté emploie un format fixe ; aucun choix de format n’est transmis : crates/sheaf-app/src/reader_ui/edits.rs:282
C-annotate-021: AGREE 9
C-annotate-022: DISAGREE f8 a9 p9 — Seules cinq couleurs prédéfinies sont proposées, sans sélecteur personnalisé : crates/sheaf-app/ui/screens/reader-options.slint:171
C-annotate-025: AGREE 9
C-annotate-029: AGREE 9
C-annotate-030: AGREE 9
C-annotate-031: DISAGREE f8 a9 p9 — Le filtre par statut d’Acrobat manque ; le panneau ne filtre que par grandes catégories de commentaires : crates/sheaf-app/ui/screens/reader-panel.slint:260
C-annotate-032: AGREE 9
C-annotate-033: DISAGREE f8 a9 p9 — Les réponses sont affichées comme un seul texte et ne peuvent pas être supprimées séparément : crates/sheaf-app/ui/screens/reader-panel.slint:332
C-annotate-034: AGREE 9
C-annotate-035: AGREE 9
C-annotate-036: AGREE 9
SUMMARY: 16 agree, 8 disagreeC-reader-001: AGREE 9  
C-reader-002: AGREE 9  
C-reader-005: AGREE 9  
C-reader-006: AGREE 9  
C-reader-007: DISAGREE f8 a9 p9 — « Fit page » s’arrête à 25 % ; une page très haute ne tient donc pas entière dans la fenêtre (crates/sheaf-app/src/viewer.rs:392).  
C-reader-008: AGREE 9  
C-reader-009: AGREE 9  
C-reader-013: DISAGREE f8 a9 p9 — Le zoom et la page sont mémorisés par document, mais le zoom par défaut reste fixé à « Fit width » sans préférence réglable (crates/sheaf-app/src/reader_ui/mod.rs:395).  
C-reader-015: DISAGREE f8 a9 p9 — Les spreads imposent l’ordre gauche-droite ; le mode droite-gauche prévu dans la définition manque (crates/sheaf-app/src/viewer.rs:321).  
C-reader-016: AGREE 9  
C-reader-018: AGREE 9  
C-reader-019: AGREE 9  
C-reader-020: AGREE 9  
C-reader-021: DISAGREE f7 a9 p9 — L’historique ne garde que le numéro de page : revenir après un saut perd la position précise dans cette page (crates/sheaf-app/src/reader_ui/nav.rs:48).  
C-reader-022: DISAGREE f8 a9 p9 — Le panneau s’ouvre et se ferme, mais sa largeur reste fixée à 248 px ; il ne se redimensionne pas (crates/sheaf-app/ui/screens/reader.slint:38).  
C-reader-023: AGREE 9  
C-reader-025: DISAGREE f8 a9 p9 — Un clic sur une entrée du plan ignore sa destination précise et revient en haut de la page, même si le signet vise un point plus bas (crates/sheaf-app/src/reader_ui/tools.rs:316).  
C-reader-026: DISAGREE f6 a9 p9 — L’interface crée seulement un signet de la page courante ; elle ne permet ni de partir du texte sélectionné, ni de changer sa destination ou son ordre (crates/sheaf-app/src/reader_ui/tools.rs:284).  
SUMMARY: 11 agree, 7 disagreeC-reader-027: AGREE 9  
C-reader-028: DISAGREE f7 a9 p9 — Un surlignage importé sans `/Contents` n’affiche pas son texte si la page n’a pas déjà été chargée : crates/sheaf-app/src/reader_ui/model.rs:479  
C-reader-030: DISAGREE f7 a9 p9 — Un clic sur une pièce jointe ouvre « Enregistrer » ; le panneau ne permet ni de l’ouvrir ni de la rechercher : crates/sheaf-app/ui/screens/reader-panel.slint:224  
C-reader-031: AGREE 9  
C-reader-032: DISAGREE f9 a9 p8 — Le test de performance est ignoré et n’affirme ni les 12,6 s ni l’arrivée du premier résultat sous 0,5 s : crates/sheaf-app/src/reader_doc.rs:592  
C-reader-033: DISAGREE f8 a9 p9 — Pendant la recherche, Entrée relance celle-ci au lieu de passer au résultat suivant : crates/sheaf-app/ui/screens/reader-toolbar.slint:234  
C-reader-034: AGREE 9  
C-reader-035: DISAGREE f7 a9 p9 — La liste s’arrête à 2 000 résultats, même si davantage de correspondances ont été trouvées : crates/sheaf-app/src/reader_ui/model.rs:272  
C-reader-043: AGREE 9  
C-reader-046: DISAGREE f7 a9 p9 — L’ouverture gère des onglets dans une seule fenêtre ; déplacer ou fusionner des fenêtres manque : crates/sheaf-app/src/reader_ui/mod.rs:466  
C-reader-047: AGREE 9  
C-reader-048: AGREE 9  
C-reader-049: AGREE 9  
C-reader-051: AGREE 9  
C-reader-052: DISAGREE f7 a9 p9 — Les propriétés montrent la taille de la première page, sans les boîtes de page ni le statut PDF/A : crates/sheaf-app/src/reader_doc.rs:142  
C-reader-061: DISAGREE f6 a9 p9 — Les étiquettes existantes servent à naviguer, mais aucune commande ne permet de marquer une page comme couverture ou de définir sa numérotation : crates/sheaf-app/src/reader_ui/nav.rs:73  
C-reader-063: DISAGREE f8 a9 p9 — Après déverrouillage, « Security » indique seulement « Encrypted » ; les restrictions du PDF ne sont pas affichées : crates/sheaf-app/src/reader_doc.rs:149  
SUMMARY: 8 agree, 9 disagreeC-edit-001: AGREE 9  
C-edit-002: DISAGREE f8 a9 p9 — Vider un paragraphe puis faire ⌘S ne valide pas sa suppression : le texte vide est ignoré dans crates/sheaf-app/src/reader_ui/saving.rs:9.  
C-edit-003: AGREE 9  
C-edit-006: AGREE 9  
C-edit-007: AGREE 9  
C-edit-011: AGREE 9  
C-edit-014: DISAGREE f8 a9 p9 — « Save As » n’applique pas le paragraphe encore ouvert dans l’éditeur avant de copier le PDF : crates/sheaf-app/src/reader_ui/saving.rs:20.  
C-edit-016: AGREE 9  
C-edit-018: DISAGREE f7 a9 p9 — Le composant promet de modifier ou retirer un filigrane existant, mais le traitement ajoute toujours un nouveau filigrane (`replace: false`) : crates/sheaf-core/src/watermark.rs:191.  
C-edit-021: DISAGREE f8 a9 p9 — Un lien `file://` peut être créé, mais un clic ne l’ouvre pas : seuls les liens HTTP(S) et `mailto:` sont traités dans crates/sheaf-app/src/reader_ui/pointer.rs:492.  
C-edit-026: AGREE 9  
C-organize-002: AGREE 9  
C-organize-003: AGREE 9  
C-organize-007: AGREE 9  
C-organize-009: AGREE 9  
C-organize-013: DISAGREE f7 a9 p9 — L’action ne fait que dupliquer les pages dans le même document ; elle ne permet pas de les copier vers un autre document comme le prévoit la définition : crates/sheaf-app/src/reader_ui/pages.rs:53.  
C-organize-014: DISAGREE f7 a9 p9 — L’extraction produit un seul PDF et laisse toujours la source intacte ; les options « un fichier par page » et « retirer de la source » manquent : crates/sheaf-app/src/reader_ui/pages.rs:93.  
C-organize-023: AGREE 9  
C-organize-027: AGREE 9  
SUMMARY: 13 agree, 6 disagreeC-files-001: AGREE 9
C-files-003: AGREE 9
C-files-004: AGREE 9
C-files-005: AGREE 9
C-files-010: AGREE 9
C-files-011: DISAGREE f8 a9 p9 — “Close Other Tabs” leaves edited documents open without offering Save / Don’t Save / Cancel; `crates/sheaf-app/src/flow.rs:501`.
C-files-012: AGREE 9
C-files-013: AGREE 9
C-files-014: AGREE 9
C-files-015: DISAGREE f8 a9 p9 — Compare cannot take PDFs already open in reader tabs; `crates/sheaf-app/src/flow.rs:85`.
C-files-016: DISAGREE f8 a9 p9 — Sign rejects a protected PDF instead of asking for its password; `crates/sheaf-app/src/files.rs:287`.
C-files-019: AGREE 9
C-batch-007: AGREE 9
C-compare-002: AGREE 9
C-compare-007: DISAGREE f7 a9 p9 — The change report compares words only, so changed images in scanned PDFs are absent from it; `crates/sheaf-core/src/compare.rs:111`.
SUMMARY: 11 agree, 4 disagreeC-convert-003: AGREE 9  
C-convert-009: DISAGREE f8 a9 p9 — une page avec texte tourné ou filigrane devient une série de zones de texte ; même ses tableaux cessent d’être des tableaux Word éditables (`third_party/pdfcraft-export/src/lib.rs:1201`).  
C-convert-012: DISAGREE f8 a9 p9 — l’outil exporte les pages rendues, mais n’offre pas le mode « Extract images » d’iLovePDF (`crates/sheaf-app/ui/screens/options.slint:574`).  
C-convert-020: DISAGREE f6 a8 p9 — aucun choix local/serveur ; l’information « rien n’est envoyé » n’apparaît que pour la conversion Office (`crates/sheaf-app/ui/screens/options.slint:909`).  
C-optimize-002: AGREE 9  
C-optimize-008: AGREE 9  
C-output-001: DISAGREE f8 a9 p9 — ⌘S ignore un paragraphe vidé dans l’éditeur : le filtre écarte le texte vide avant de valider la modification (`crates/sheaf-app/src/reader_ui/saving.rs:9`).  
C-output-005: DISAGREE f8 a8 p9 — après un lot partiellement échoué, le bandeau annonce quand même une réussite complète, bien que les échecs soient listés (`crates/sheaf-app/src/state.rs:416`).  
C-output-006: AGREE 9  
C-platform-009: AGREE 9  
C-platform-013: AGREE 9  
C-preferences-001: AGREE 9  
C-preferences-002: DISAGREE f7 a9 p9 — les choix système/clair/sombre existent, mais la couleur d’accent et celle du fond de fenêtre demandées ne sont pas réglables (`crates/sheaf-app/ui/screens/preferences.slint:125`).  
C-preferences-008: DISAGREE f8 a9 p9 — le nom choisi sert aux annotations ; il ne renseigne pas l’auteur des documents, modifié séparément dans leurs propriétés (`crates/sheaf-app/src/home.rs:319`).  
C-preferences-009: AGREE 9  

SUMMARY: 8 agree, 7 disagreeC-security-002: AGREE 9  
C-security-004: DISAGREE f7 a9 p9 — Impossible de limiter les droits sans imposer aussi un mot de passe d’ouverture, contrairement à Acrobat : crates/sheaf-app/ui/screens/options.slint:251  
C-security-005: AGREE 9  
C-security-006: AGREE 9  
C-security-010: AGREE 9  
C-security-012: DISAGREE f8 a9 p9 — Une ancienne recherche peut écraser les résultats d’une nouvelle sur le même PDF ; la réponse ne vérifie que l’identifiant du fichier : crates/sheaf-app/src/viewer_ui.rs:663  
C-security-017: DISAGREE f7 a9 p9 — Seuls quatre champs sont modifiables ; créateur, producteur, dates et propriétés personnalisées ne le sont pas : crates/sheaf-app/src/reader_ui/properties.rs:19  
C-sign-001: AGREE 9  
C-sign-002: AGREE 9  
C-sign-003: AGREE 9  
C-sign-005: AGREE 9  
C-sign-008: AGREE 9  
C-sign-009: DISAGREE f7 a9 p9 — Les signatures sont conservées dans prefs.json, sans stockage sécurisé ni option d’usage unique : crates/sheaf-app/src/prefs.rs:131  
C-sign-010: DISAGREE f8 a9 p9 — Les marques prennent leur couleur par défaut à la pose ; la couleur choisie n’est pas appliquée : crates/sheaf-app/src/reader_ui/edits.rs:271  
C-forms-001: AGREE 9  
C-forms-002: DISAGREE f8 a9 p9 — « Clear form » efface immédiatement, sans la confirmation prévue par la définition : crates/sheaf-app/ui/screens/reader-options.slint:182  
C-forms-005: DISAGREE f8 a9 p9 — La teinte des champs est automatique en mode Select ; aucune commande ne permet de l’activer ou de la désactiver : crates/sheaf-app/src/reader_ui/model.rs:132  
C-forms-017: AGREE 9  
SUMMARY: 11 agree, 7 disagree