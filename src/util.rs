use bcrypt::{hash, verify, BcryptError, DEFAULT_COST};

pub fn hash_password(plain: String) -> Result<String, BcryptError> {
    hash(plain, DEFAULT_COST)
}
pub fn verify_password(plain: &str, hash: &str) -> Result<bool, BcryptError> {
    verify(plain, hash)
}
