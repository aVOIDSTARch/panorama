use axum::{
    extract::Form,
    http::StatusCode,
    response::{Html, IntoResponse},
};
use serde::Deserialize;

/// Login page (simple HTML form — to be replaced with WebAuthn).
pub async fn login_page() -> impl IntoResponse {
    Html(LOGIN_HTML)
}

#[derive(Deserialize)]
pub struct LoginForm {
    pub password: String,
}

/// Handle login form submission.
pub async fn login_submit(Form(form): Form<LoginForm>) -> impl IntoResponse {
    let expected = std::env::var("ADMIN_PASSWORD").unwrap_or_else(|_| "panorama".into());

    if form.password == expected {
        (
            StatusCode::SEE_OTHER,
            [
                ("location", "/"),
                (
                    "set-cookie",
                    "admin_session=active; Path=/; HttpOnly; SameSite=Strict",
                ),
            ],
            "",
        )
            .into_response()
    } else {
        (StatusCode::UNAUTHORIZED, Html("<h2>Invalid password</h2><a href='/login'>Try again</a>")).into_response()
    }
}

const LOGIN_HTML: &str = r#"<!DOCTYPE html>
<html><head><title>Panorama Admin</title>
<style>body{font-family:system-ui;display:flex;justify-content:center;align-items:center;height:100vh;margin:0;background:#1a1a2e}
form{background:#16213e;padding:2rem;border-radius:8px;color:#e0e0e0}
input{display:block;margin:0.5rem 0;padding:0.5rem;width:200px;border:1px solid #555;background:#0f3460;color:#fff;border-radius:4px}
button{margin-top:1rem;padding:0.5rem 1.5rem;background:#e94560;color:#fff;border:none;border-radius:4px;cursor:pointer}
</style></head><body>
<form method="post" action="/login">
<h2>Panorama Admin</h2>
<label>Password</label>
<input type="password" name="password" autofocus>
<button type="submit">Login</button>
</form></body></html>"#;
