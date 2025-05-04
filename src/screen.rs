pub mod vault;

pub use vault::Vault;

pub enum Screen {
    Vault(Vault),
}
