// Pages des Paramètres Windows (URI `ms-settings:` documentées par Microsoft) et quelques
// outils classiques. Catalogue écrit pour Island : titre FR, titre EN, alias de recherche
// (les deux langues sont toujours cherchables, le titre suit la langue d'Island).

use crate::launcher::model::{Item, Kind};

/// (cible, titre fr, titre en, alias)
const PAGES: &[(&str, &str, &str, &str)] = &[
    // Système
    ("ms-settings:display", "Écran", "Display", "affichage resolution luminosite brightness hdr echelle scale moniteur monitor"),
    ("ms-settings:nightlight", "Éclairage nocturne", "Night light", "lumiere bleue blue light nuit"),
    ("ms-settings:sound", "Son", "Sound", "audio volume haut-parleur speaker micro microphone sortie output"),
    ("ms-settings:apps-volume", "Mélangeur de volume", "Volume mixer", "volume par application app volume"),
    ("ms-settings:notifications", "Notifications", "Notifications", "alertes toasts"),
    ("ms-settings:quiethours", "Ne pas déranger", "Do not disturb", "focus assist concentration silence"),
    ("ms-settings:powersleep", "Alimentation et veille", "Power & sleep", "batterie battery mise en veille sleep ecran eteint"),
    ("ms-settings:batterysaver", "Économiseur de batterie", "Battery saver", "batterie energie energy"),
    ("ms-settings:storagesense", "Stockage", "Storage", "disque disk espace space nettoyage cleanup"),
    ("ms-settings:multitasking", "Multitâche", "Multitasking", "ancrer snap fenetres windows bureaux virtuels virtual desktops alt tab"),
    ("ms-settings:project", "Projection", "Projecting to this PC", "projeter miracast"),
    ("ms-settings:clipboard", "Presse-papiers", "Clipboard", "historique copier coller win v"),
    ("ms-settings:remotedesktop", "Bureau à distance", "Remote Desktop", "rdp distance"),
    ("ms-settings:about", "Informations système", "About", "a propos nom du pc rename pc specifications version windows"),
    ("ms-settings:activation", "Activation", "Activation", "licence license cle product key"),
    ("ms-settings:troubleshoot", "Résolution des problèmes", "Troubleshoot", "depannage depanner probleme"),
    ("ms-settings:recovery", "Récupération", "Recovery", "reinitialiser reset restaurer"),
    ("ms-settings:optionalfeatures", "Fonctionnalités facultatives", "Optional features", "fonctionnalites features"),
    // Appareils
    ("ms-settings:bluetooth", "Bluetooth et appareils", "Bluetooth & devices", "appareils devices casque headset souris mouse clavier keyboard"),
    ("ms-settings:printers", "Imprimantes et scanners", "Printers & scanners", "imprimante printer scanner"),
    ("ms-settings:mousetouchpad", "Souris", "Mouse", "pointeur pointer sensibilite"),
    ("ms-settings:devices-touchpad", "Pavé tactile", "Touchpad", "trackpad gestes gestures"),
    ("ms-settings:typing", "Saisie", "Typing", "correcteur orthographe spelling clavier tactile"),
    ("ms-settings:pen", "Stylet et Windows Ink", "Pen & Windows Ink", "stylet pen"),
    ("ms-settings:autoplay", "Exécution automatique", "AutoPlay", "autoplay usb"),
    ("ms-settings:usb", "USB", "USB", "usb"),
    ("ms-settings:camera", "Caméras", "Cameras", "webcam camera"),
    // Réseau
    ("ms-settings:network-status", "Réseau et Internet", "Network & internet", "reseau network internet connexion"),
    ("ms-settings:network-wifi", "Wi-Fi", "Wi-Fi", "wifi sans fil wireless"),
    ("ms-settings:network-ethernet", "Ethernet", "Ethernet", "cable lan"),
    ("ms-settings:network-vpn", "VPN", "VPN", "vpn"),
    ("ms-settings:network-mobilehotspot", "Point d'accès mobile", "Mobile hotspot", "partage de connexion hotspot"),
    ("ms-settings:network-airplanemode", "Mode Avion", "Airplane mode", "avion airplane"),
    ("ms-settings:network-proxy", "Proxy", "Proxy", "proxy"),
    ("ms-settings:network-advancedsettings", "Paramètres réseau avancés", "Advanced network settings", "carte reseau adapter dns"),
    // Personnalisation
    ("ms-settings:personalization-background", "Arrière-plan", "Background", "fond d'ecran wallpaper image"),
    ("ms-settings:colors", "Couleurs", "Colors", "theme sombre dark clair light accent couleur"),
    ("ms-settings:themes", "Thèmes", "Themes", "theme"),
    ("ms-settings:lockscreen", "Écran de verrouillage", "Lock screen", "verrouillage lock"),
    ("ms-settings:taskbar", "Barre des tâches", "Taskbar", "barre des taches taskbar icones"),
    ("ms-settings:personalization-start", "Menu Démarrer", "Start", "demarrer start menu"),
    ("ms-settings:fonts", "Polices", "Fonts", "police font"),
    // Applications
    ("ms-settings:appsfeatures", "Applications installées", "Installed apps", "desinstaller uninstall programmes programs applications"),
    ("ms-settings:defaultapps", "Applications par défaut", "Default apps", "navigateur par defaut default browser ouvrir avec"),
    ("ms-settings:startupapps", "Applications au démarrage", "Startup apps", "demarrage startup lancement"),
    ("ms-settings:optionalfeatures", "Fonctionnalités facultatives", "Optional features", ""),
    // Comptes
    ("ms-settings:yourinfo", "Vos informations", "Your info", "compte account profil"),
    ("ms-settings:signinoptions", "Options de connexion", "Sign-in options", "mot de passe password pin windows hello empreinte visage"),
    ("ms-settings:emailandaccounts", "E-mail et comptes", "Email & accounts", "mail comptes accounts"),
    ("ms-settings:otherusers", "Autres utilisateurs", "Other users", "famille family utilisateur user"),
    ("ms-settings:sync", "Sauvegarde Windows", "Windows backup", "synchronisation sync onedrive sauvegarde"),
    // Heure et langue
    ("ms-settings:dateandtime", "Date et heure", "Date & time", "heure time fuseau horaire timezone horloge clock"),
    ("ms-settings:regionlanguage", "Langue et région", "Language & region", "langue language region clavier keyboard"),
    ("ms-settings:speech", "Voix", "Speech", "reconnaissance vocale speech dictee"),
    // Jeux
    ("ms-settings:gaming-gamebar", "Xbox Game Bar", "Xbox Game Bar", "game bar jeux gaming"),
    ("ms-settings:gaming-gamemode", "Mode Jeu", "Game Mode", "jeu game performances"),
    ("ms-settings:gaming-gamedvr", "Captures", "Captures", "enregistrement record clips"),
    // Accessibilité
    ("ms-settings:easeofaccess-display", "Taille du texte", "Text size", "texte text accessibilite accessibility"),
    ("ms-settings:easeofaccess-magnifier", "Loupe", "Magnifier", "zoom loupe"),
    ("ms-settings:easeofaccess-highcontrast", "Thèmes de contraste", "Contrast themes", "contraste contrast"),
    ("ms-settings:easeofaccess-narrator", "Narrateur", "Narrator", "lecteur d'ecran screen reader"),
    ("ms-settings:easeofaccess-keyboard", "Clavier (accessibilité)", "Keyboard (accessibility)", "touches remanentes sticky keys clavier visuel on-screen keyboard"),
    // Confidentialité et sécurité
    ("ms-settings:privacy", "Confidentialité et sécurité", "Privacy & security", "confidentialite privacy securite security"),
    ("ms-settings:privacy-location", "Localisation", "Location", "position gps location"),
    ("ms-settings:privacy-webcam", "Accès à la caméra", "Camera access", "camera webcam autorisations permissions"),
    ("ms-settings:privacy-microphone", "Accès au micro", "Microphone access", "micro microphone autorisations permissions"),
    ("ms-settings:privacy-notifications", "Accès aux notifications", "Notification access", "notifications autorisations"),
    ("ms-settings:windowsdefender", "Sécurité Windows", "Windows Security", "antivirus defender pare-feu firewall"),
    ("ms-settings:findmydevice", "Localiser mon appareil", "Find my device", "localiser"),
    // Windows Update
    ("ms-settings:windowsupdate", "Windows Update", "Windows Update", "mise a jour update mises a jour"),
    ("ms-settings:windowsupdate-history", "Historique des mises à jour", "Update history", "historique history"),
    ("ms-settings:windowsupdate-options", "Options avancées de Windows Update", "Windows Update advanced options", "options"),
];

/// Outils classiques (lancés tels quels).
const TOOLS: &[(&str, &str, &str, &str)] = &[
    ("control.exe", "Panneau de configuration", "Control Panel", "panneau control panel"),
    ("devmgmt.msc", "Gestionnaire de périphériques", "Device Manager", "pilotes drivers peripheriques devices"),
    ("taskmgr.exe", "Gestionnaire des tâches", "Task Manager", "processus process taches ctrl shift echap"),
    ("services.msc", "Services", "Services", "services"),
    ("diskmgmt.msc", "Gestion des disques", "Disk Management", "partitions disques disks"),
    ("regedit.exe", "Éditeur du Registre", "Registry Editor", "registre registry regedit"),
    ("eventvwr.msc", "Observateur d'événements", "Event Viewer", "evenements events logs journaux"),
    ("rundll32.exe sysdm.cpl,EditEnvironmentVariables", "Variables d'environnement", "Environment variables", "path variables env"),
    ("ncpa.cpl", "Connexions réseau", "Network Connections", "cartes reseau adapters"),
    ("mmsys.cpl", "Périphériques audio (classique)", "Sound devices (classic)", "son sound lecture enregistrement"),
    ("appwiz.cpl", "Programmes et fonctionnalités", "Programs and Features", "desinstaller uninstall"),
    ("cleanmgr.exe", "Nettoyage de disque", "Disk Cleanup", "nettoyer clean"),
    ("msinfo32.exe", "Informations système (détaillées)", "System Information", "materiel hardware"),
    ("resmon.exe", "Moniteur de ressources", "Resource Monitor", "ressources resources"),
    ("taskschd.msc", "Planificateur de tâches", "Task Scheduler", "taches planifiees scheduled tasks"),
];

pub fn items(lang: &str) -> Vec<Item> {
    let fr = lang.starts_with("fr");
    let mut seen = std::collections::HashSet::new();
    PAGES
        .iter()
        .chain(TOOLS.iter())
        .filter(|(target, ..)| seen.insert(*target))
        .map(|(target, t_fr, t_en, kw)| {
            let (title, other) = if fr { (*t_fr, *t_en) } else { (*t_en, *t_fr) };
            Item {
                id: format!("set:{target}"),
                kind: Kind::Setting,
                title: title.to_string(),
                subtitle: String::new(),
                target: target.to_string(),
                keywords: format!("{other} {kw}"),
            }
        })
        .collect()
}
