//! Formats d'Island partagés par l'app (`src-tauri`) et le serveur du magasin (`server`) :
//! les mêmes règles de validation des deux côtés, pour qu'un fichier accepté par l'un le
//! soit par l'autre.
//!
//! - [`scene`] : scènes de fonds animés (calques, effets, masques) ;
//! - [`islandscene`] : fichier de partage `.islandscene` (lecture vérifiée, écriture) ;
//! - [`package`] : paquets d'extension `.island` (manifeste, empreinte, signature).
//!
//! Aucune dépendance à Tauri ni à l'OS : tout part d'octets ou de lecteurs.

pub mod islandscene;
pub mod package;
pub mod scene;
