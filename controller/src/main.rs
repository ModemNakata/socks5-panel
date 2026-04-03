use salvo::affix_state;
use salvo::prelude::*;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

#[derive(Clone)]
struct AppState {
    secret_key: String,
    passwd_path: PathBuf,
    write_lock: Arc<Mutex<()>>,
}

#[derive(Serialize)]
struct ApiResponse {
    message: String,
}

fn load_env(path: &str) -> (String, PathBuf) {
    let content = fs::read_to_string(path).expect("Failed to read .unit_variables");
    let mut secret_key = None;
    let mut passwd_path = None;

    for line in content.lines() {
        let line = line.trim();
        if line.starts_with("SERVICE_2_SERVICE_PRE_SHARED_SECRET_KEY=") {
            secret_key = Some(
                line.trim_start_matches("SERVICE_2_SERVICE_PRE_SHARED_SECRET_KEY=")
                    .to_string(),
            );
        } else if line.starts_with("PASSWD_PATH=") {
            passwd_path = Some(line.trim_start_matches("PASSWD_PATH=").to_string());
        }
    }

    (
        secret_key.expect("SERVICE_2_SERVICE_PRE_SHARED_SECRET_KEY not found"),
        PathBuf::from(passwd_path.expect("PASSWD_PATH not found")),
    )
}

// apt install whois
fn generate_password_hash(password: &str) -> String {
    let output = std::process::Command::new("mkpasswd")
        .args(["--method=md5", password])
        .output()
        .expect("Failed to execute mkpasswd")
        .stdout;
    String::from_utf8_lossy(&output).trim().to_string()
}

fn read_users(path: &PathBuf) -> Vec<String> {
    if path.exists() {
        fs::read_to_string(path)
            .unwrap_or_default()
            .lines()
            .map(|s| s.to_string())
            .collect()
    } else {
        Vec::new()
    }
}

fn write_users(path: &PathBuf, users: &[String]) -> Result<(), String> {
    fs::write(path, users.join("\n")).map_err(|e| e.to_string())
}

#[handler]
async fn check_auth(req: &mut Request, depot: &mut Depot) -> Result<(), StatusError> {
    let state = depot.obtain::<AppState>().unwrap();
    let auth_header = req
        .headers()
        .get("Authorization")
        .and_then(|v| v.to_str().ok());

    let provided_secret = if let Some(auth_val) = auth_header {
        if let Some(prefix) = auth_val.strip_prefix("Bearer ") {
            prefix
        } else {
            return Err(StatusError::unauthorized());
        }
    } else {
        return Err(StatusError::unauthorized());
    };

    if provided_secret == state.secret_key {
        depot.insert("authorized", true);
        Ok(())
    } else {
        Err(StatusError::unauthorized())
    }
}

#[handler]
async fn create_user(
    req: &mut Request,
    depot: &mut Depot,
) -> Result<Json<ApiResponse>, StatusError> {
    if !depot.get::<bool>("authorized").copied().unwrap_or(false) {
        return Err(StatusError::unauthorized());
    }

    let state = depot.obtain::<AppState>().unwrap();

    #[derive(Deserialize)]
    struct CreateUserRequest {
        username: String,
        password: String,
    }

    let create_req = req
        .parse_json::<CreateUserRequest>()
        .await
        .map_err(|_| StatusError::bad_request())?;

    let password_hash = generate_password_hash(&create_req.password);
    let entry = format!("{}:{}", create_req.username, password_hash);

    let _lock = state.write_lock.lock().unwrap();
    let mut users = read_users(&state.passwd_path);
    if users
        .iter()
        .any(|u| u.starts_with(&format!("{}:", create_req.username)))
    {
        return Err(StatusError::bad_request());
    }

    users.push(entry);
    write_users(&state.passwd_path, &users).map_err(|_| StatusError::internal_server_error())?;

    Ok(Json(ApiResponse {
        message: format!("User {} created successfully", create_req.username),
    }))
}

#[handler]
async fn delete_user(
    req: &mut Request,
    depot: &mut Depot,
) -> Result<Json<ApiResponse>, StatusError> {
    if !depot.get::<bool>("authorized").copied().unwrap_or(false) {
        return Err(StatusError::unauthorized());
    }

    let state = depot.obtain::<AppState>().unwrap();

    #[derive(Deserialize)]
    struct DeleteUserRequest {
        username: String,
    }

    let delete_req = req
        .parse_json::<DeleteUserRequest>()
        .await
        .map_err(|_| StatusError::bad_request())?;

    let _lock = state.write_lock.lock().unwrap();
    let mut users = read_users(&state.passwd_path);
    let original_len = users.len();
    users.retain(|u| !u.starts_with(&format!("{}:", delete_req.username)));

    if users.len() == original_len {
        return Err(StatusError::not_found());
    }

    write_users(&state.passwd_path, &users).map_err(|_| StatusError::internal_server_error())?;

    Ok(Json(ApiResponse {
        message: format!("User {} deleted successfully", delete_req.username),
    }))
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt().init();

    let (secret_key, passwd_path) = load_env(".unit_variables");

    let state = AppState {
        secret_key,
        passwd_path,
        write_lock: Arc::new(Mutex::new(())),
    };

    let router = Router::new().hoop(affix_state::inject(state.clone())).push(
        Router::with_hoop(check_auth).push(
            Router::new().push(
                Router::with_path("/user")
                    .post(create_user)
                    .delete(delete_user),
            ),
        ),
    );

    println!("{router:?}");

    let acceptor = TcpListener::new("127.0.0.1:8698").bind().await;
    Server::new(acceptor).serve(router).await;
}

//
// static linking with musl:
// sudo dnf install -y musl-gcc
//
// rustup target add x86_64-unknown-linux-musl
//
// cargo build --release --target x86_64-unknown-linux-musl
//
