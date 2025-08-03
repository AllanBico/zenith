use hmac::{Hmac, Mac};
use sha2::Sha256;

// Create a type alias for the HMAC-SHA256 implementation.
type HmacSha256 = Hmac<Sha256>;

/// Creates an HMAC-SHA256 signature for a given query string.
///
/// Binance requires all private API calls to be signed. This function implements
/// the required signing logic according to their documentation.
///
/// # Arguments
///
/// * `secret` - The user's API secret key.
/// * `query_string` - The full query string of the request, including the timestamp.
///
/// # Returns
///
/// A hexadecimal string representation of the signature.
pub fn sign_request(secret: &str, query_string: &str) -> String {
    // Create a new HMAC-SHA256 instance with the secret key.
    let mut mac = HmacSha256::new_from_slice(secret.as_bytes())
        .expect("HMAC can take key of any size");

    // Update the HMAC instance with the data to be signed (the query string).
    mac.update(query_string.as_bytes());

    // Finalize the HMAC computation and get the raw byte result.
    let result = mac.finalize();
    let code_bytes = result.into_bytes();

    // Convert the raw bytes into a hexadecimal string, which is what the API expects.
    hex::encode(code_bytes)
}