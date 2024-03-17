use aws_lambda::lambda;
use aws_lambda::event::apigw::{ApiGatewayProxyRequest, ApiGatewayProxyResponse};
use aws_lambda::error::HandlerError;
use log::{info, error};
use serde_json::{json, Value};
use rusoto_core::Region;
use rusoto_dynamodb::{DynamoDb, DynamoDbClient, GetItemInput};
use rusoto_xray::{PutTraceSegmentsRequest, Segment, XRay, XRayClient};

#[tokio::main]
async fn main() -> Result<(), HandlerError> {
    simple_logger::init_with_level(log::Level::Info).unwrap();
    lambda!(handler);
    Ok(())
}

async fn handler(request: ApiGatewayProxyRequest) -> Result<ApiGatewayProxyResponse, HandlerError> {
    let operation = request.http_method();

    // Initialize X-Ray client
    let xray_client = XRayClient::new(Region::default());

    // Create a segment
    let segment = Segment {
        id: Some("my-segment".to_string()),
        ..Default::default()
    };

    // Put the segment
    match xray_client.put_trace_segments(PutTraceSegmentsRequest {
        trace_segment_documents: vec![segment.document.unwrap()],
    }).await {
        Ok(_) => info!("Successfully recorded X-Ray segment"),
        Err(e) => error!("Failed to record X-Ray segment: {:?}", e),
    };

    if operation == "OPTIONS" {
        // Return a 200 OK response with CORS headers
        return Ok(api_gateway_response(200, json!({"message": "CORS preflight response"})));
    }

    // Parse the incoming JSON payload
    let body = match request.body() {
        Some(body) => body,
        None => return Ok(api_gateway_response(400, json!({"message": "Invalid request body"}))),
    };

    let parsed_body: Value = match serde_json::from_str(&body) {
        Ok(value) => value,
        Err(_) => return Ok(api_gateway_response(400, json!({"message": "Invalid JSON payload"}))),
    };

    let bar_name = match parsed_body.get("barName") {
        Some(value) => match value.as_str() {
            Some(value) => value,
            None => return Ok(api_gateway_response(400, json!({"message": "barName is not a string"}))),
        },
        None => return Ok(api_gateway_response(400, json!({"message": "barName is missing"}))),
    };

    let drink_name = match parsed_body.get("drinkName") {
        Some(value) => match value.as_str() {
            Some(value) => value,
            None => return Ok(api_gateway_response(400, json!({"message": "drinkName is not a string"}))),
        },
        None => return Ok(api_gateway_response(400, json!({"message": "drinkName is missing"}))),
    };

    // Log bar name and drink name
    info!("Received request for Bar: {}, Drink: {}", bar_name, drink_name);

    // Initialize DynamoDB client
    let dynamodb_client = DynamoDbClient::new(Region::default());

    // Query DynamoDB using the Bar Name and Drink Name as keys
    let input = GetItemInput {
        table_name: String::from("drink_images"),
        key: json!({
            "barName": { "S": bar_name },
            "drinkName": { "S": drink_name }
        }),
        ..Default::default()
    };

    match dynamodb_client.get_item(input).await {
        Ok(response) => {
            match response.item {
                Some(item) => {
                    // Extract the ObjectURL attribute
                    let object_url = match item.get("s3ObjectKey") {
                        Some(value) => match value.s {
                            Some(url) => url,
                            None => "URL not found".to_string(),
                        },
                        None => "URL not found".to_string(),
                    };
                    Ok(api_gateway_response(200, json!({"s3ObjectKey": object_url})))
                },
                None => Ok(api_gateway_response(404, json!({"message": "Item not found"})))
            }
        },
        Err(_) => Ok(api_gateway_response(500, json!({"message": "Internal Server Error"})))
    }
}

fn api_gateway_response(status_code: u16, body: Value) -> ApiGatewayProxyResponse {
    ApiGatewayProxyResponse {
        status_code,
        headers: Default::default(),
        multi_value_headers: Default::default(),
        body: Some(body.to_string()),
        is_base64_encoded: Some(false),
    }
}
