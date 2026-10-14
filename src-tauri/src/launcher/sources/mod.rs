// Sources de l'index du launcher. Chacune renvoie ses éléments ; l'index les assemble et
// déduplique. Les fournisseurs des extensions viendront s'ajouter ici (slot « launcher »).

pub mod apps;
pub mod files;
pub mod settings;
