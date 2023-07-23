mod premium;
use log::{error, info};
use premium::*;
use serde::Serialize;
use tide::{Body, Request, Response, StatusCode};

#[async_std::main]
async fn main() -> tide::Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .format_timestamp(None)
        .format_module_path(false)
        .init();

    let mut app = tide::new();
    app.at("/").get(healthz);
    app.at("/api/v1/healths/premiums").post(premiums);
    app.at("/api/v1/healths/premiums/loads").post(load_matrix);
    app.at("/api/v1/healths/premiums/unloads")
        .post(unload_matrix);
    app.at("/api/v1/healths/premiums/checks").get(check_matrix);
    let _listener = app.listen("127.0.0.1:8000").await?;
    info!("premium api started");
    Ok(())
}

async fn healthz(_req: Request<()>) -> tide::Result {
    let response = Response::new(StatusCode::Ok);
    Ok(response)
}

async fn premiums(mut req: Request<()>) -> tide::Result {
    let request: HealthRequest;
    match validate_parse_request(&mut req).await {
        Ok(result) => request = result,
        Err(err) => return Ok(handle_error(err)),
    };

    let health_response = calculate_premium(request).await;
    match health_response {
        Ok(premium) => Ok(make_response::<HealthResponse>(&premium.into())?),
        Err(err) => Ok(handle_error(err)),
    }
}

async fn load_matrix(_req: Request<()>) -> tide::Result {
    let result = load().await;
    match result {
        Ok(_) => Ok(Response::new(StatusCode::Ok)),
        Err(err) => Ok(handle_error(err)),
    }
}

async fn unload_matrix(_req: Request<()>) -> tide::Result {
    let result = unload().await;
    match result {
        Ok(_) => Ok(Response::new(StatusCode::Ok)),
        Err(err) => Ok(handle_error(err)),
    }
}

async fn check_matrix(_req: Request<()>) -> tide::Result {
    let result = keys_exists().await;
    match result {
        Ok(_) => Ok(Response::new(StatusCode::Ok)),
        Err(err) => Ok(handle_error(err)),
    }
}

fn handle_error(err: PremiumError) -> Response {
    match err {
        PremiumError::InternalServer => match make_json_error_response("001", err.to_string()) {
            Ok(response) => response,
            Err(_) => Response::new(StatusCode::InternalServerError),
        },
        PremiumError::InvalidInput => match make_json_error_response("002", err.to_string()) {
            Ok(response) => response,
            Err(_) => Response::new(StatusCode::InternalServerError),
        },

        PremiumError::RiskCalculation => match make_json_error_response("004", err.to_string()) {
            Ok(response) => response,
            Err(_) => Response::new(StatusCode::InternalServerError),
        },
        PremiumError::InvalidHeader(header) => {
            match make_json_error_response(
                "003",
                format!("Header {} not provided or invalid", header),
            ) {
                Ok(response) => response,
                Err(_) => Response::new(StatusCode::InternalServerError),
            }
        }
    }
}

async fn validate_parse_request(
    req: &mut Request<()>,
) -> anyhow::Result<HealthRequest, PremiumError> {
    validate_request(&req)?;
    let body = body_string(req).await?;
    let result = serde_json::from_str::<HealthRequest>(body.as_str());
    match result {
        Ok(request) => Ok(request),
        Err(err) => {
            error!(
                "Serialization error while converting json to struct {}",
                err.to_string()
            );
            Err(PremiumError::InvalidInput)
        }
    }
}
fn validate_request(request: &Request<()>) -> anyhow::Result<Response, PremiumError> {
    validate_headers(request)
}

fn validate_headers(request: &Request<()>) -> anyhow::Result<Response, PremiumError> {
    let content_type = request.header("Content-Type").map(|header| header.as_str());
    match content_type {
        Some("application/json") => Ok(Response::new(StatusCode::Ok)),
        _ => Err(PremiumError::InvalidHeader("content-type".to_string())),
    }
}

async fn body_string(req: &mut Request<()>) -> anyhow::Result<String, PremiumError> {
    let body_result = req.body_string().await;
    match body_result {
        Ok(body) => Ok(body),
        Err(err) => {
            error!("Parsing error of request body {}", err.to_string());
            Err(PremiumError::InternalServer)
        }
    }
}

fn make_json_error_response(err_code: &str, message: String) -> tide::Result {
    let err = ErrorResponse {
        code: err_code.to_string(),
        message: message.to_string(),
    };
    make_response(&err)
}

fn make_response<T: Serialize>(response: &T) -> tide::Result {
    let data = Body::from_json(&response);
    match data {
        Ok(data) => {
            let mut response = Response::new(StatusCode::Ok);
            response.set_body(data);
            Ok(response)
        }
        Err(err) => {
            info!("Error while converting response {:?}", err);
            Err(tide::Error::from_str(
                StatusCode::InternalServerError,
                "Internal server error",
            ))
        }
    }
}
