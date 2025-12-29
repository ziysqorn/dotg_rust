use crate::global_vars::SECRET_KEY;
use axum::{
    RequestPartsExt,
    extract::FromRequestParts,
    http::{StatusCode, request::Parts},
    response::{IntoResponse, Response},
};
use axum_extra::{
    TypedHeader,
    headers::authorization::{Authorization, Bearer},
};
use dotenvy::dotenv;
use jsonwebtoken::{DecodingKey, Validation, decode};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub subject: String,
    pub exp: usize,
}

pub struct AuthUser {
    pub username: String,
}

impl<S> FromRequestParts<S> for AuthUser
where
    S: Send + Sync,
{
    type Rejection = AuthError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let TypedHeader(Authorization(bearer)) = parts
            .extract::<TypedHeader<Authorization<Bearer>>>()
            .await
            .map_err(|_| AuthError::MissingToken)?;

        // b. Giải mã Token
        let token_data = decode::<Claims>(
            bearer.token(),
            &DecodingKey::from_secret(get_jwt_secret()),
            &Validation::default(),
        )
        .map_err(|_| AuthError::InvalidToken)?;

        // c. Nếu OK -> Trả về AuthUser chứa username
        Ok(AuthUser {
            username: token_data.claims.subject,
        })
    }
}

pub enum AuthError {
    MissingToken,
    InvalidToken,
}

impl IntoResponse for AuthError {
    fn into_response(self) -> Response {
        match self {
            AuthError::MissingToken => (StatusCode::UNAUTHORIZED, "Missing Token").into_response(),
            AuthError::InvalidToken => (StatusCode::UNAUTHORIZED, "Invalid Token").into_response(),
        }
    }
}

pub fn get_jwt_secret() -> &'static [u8] {
    let secret_key = SECRET_KEY.get_or_init(|| {
        dotenv().expect("Error loading .env file");
        return std::env::var("SECRET_KEY").expect("SECRET_KEY not found !");
    });
    return secret_key.as_bytes();
}
