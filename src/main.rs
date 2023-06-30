use log::info;
use serde::{Deserialize, Serialize};
use tide::{Body, Request, Response, StatusCode};
//use validator::{Validate, ValidationError, ValidationErrorsKind};

#[derive(Debug, Deserialize)]
struct HealthRequest {
    code: String,
    //#[validate(length(min = 1, message = "sum insured is required"))]
    #[serde(rename = "sumInsured")]
    sum_insured: String,
    #[serde(rename = "dateOfBirth")]
    date_of_birth: String,
}

#[derive(Serialize, Debug)]
struct HealthResponse {
    premium: String,
}

#[derive(Serialize, Debug)]
struct ErrorResponse {
    code: String,
    message: String,
}

#[async_std::main]
async fn main() -> tide::Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .format_timestamp(None)
        .format_module_path(false)
        .init();

    let mut app = tide::new();
    app.at("/").get(health);
    app.at("/api/v1/healths/premiums").post(premiums);
    let _listener = app.listen("127.0.0.1:8000").await?;
    info!("premium api started");
    Ok(())
}

async fn premiums(mut req: Request<()>) -> tide::Result {
    handle_request(&req).await?;
    let str_data = req.body_string().await?;
    let health_req: HealthRequest = serde_json::from_str(&str_data).map_err(handle_serde_error)?;

    info!("{:?}", health_req);
    let response = HealthResponse {
        premium: "250".to_string(),
    };

    let data = Body::from_json(&response);
    match data {
        Ok(data) => {
            println!("{:?}", response);
            let mut response = Response::new(StatusCode::Ok);
            response.set_body(data);
            Ok(response)
        }
        Err(err) => {
            info!("Error while converting response {:?}", err);
            Ok(tide::Response::new(tide::StatusCode::InternalServerError))
        }
    }
}

async fn handle_request(request: &Request<()>) -> tide::Result {
    let content_type = request.header("Content-Type").map(|header| header.as_str());
    match content_type {
        Some("application/json") => Ok(Response::new(StatusCode::Ok)),
        _ => Err(handle_request_error("content type not application/json")),
    }
}

async fn health(_req: Request<()>) -> tide::Result {
    let response = Response::new(StatusCode::Ok);
    Ok(response)
}

fn handle_serde_error(error: serde_json::Error) -> tide::Error {
    info!("serialization error {}", error.to_string());
    let error_response = ErrorResponse {
        code: "002".to_string(),
        message: error.to_string(),
    };

    //TODO do not unwrap use match
    tide::Error::from_str(
        StatusCode::InternalServerError,
        serde_json::to_string(&error_response).unwrap(),
    )
}

fn handle_request_error(message: &str) -> tide::Error {
    let error_response = ErrorResponse {
        code: "001".to_string(),
        message: message.to_string(),
    };

    tide::Error::from_str(
        StatusCode::BadRequest,
        serde_json::to_string(&error_response).unwrap(),
    )
}
