#![cfg(feature = "rocket")]

use std::{borrow::Cow, io::Cursor, sync::Arc};

use rocket::{
    http::{Header, Status},
    response::{
        status::{self, NotFound},
        Responder as RocketResponder,
    },
    route::{Handler, Outcome},
    serde::json::Json,
    Data as RocketData, Request, Response, Route,
};

use crate::{Config, SwaggerFile, SwaggerUi};

impl From<SwaggerUi> for Vec<Route> {
    fn from(swagger_ui: SwaggerUi) -> Self {
        let mut routes = Vec::<Route>::with_capacity(swagger_ui.urls.len() + 1);
        let mut api_docs = Vec::<Route>::with_capacity(swagger_ui.urls.len());

        let urls = swagger_ui.urls.into_iter().map(|(url, openapi)| {
            api_docs.push(Route::new(
                rocket::http::Method::Get,
                url.url.as_ref(),
                ServeApiDoc(openapi),
            ));
            url
        });

        routes.push(Route::new(
            rocket::http::Method::Get,
            swagger_ui.path.as_ref(),
            ServeSwagger(
                swagger_ui.path.clone(),
                Arc::new(Config {
                    urls: urls.collect(),
                    oauth: swagger_ui.oauth,
                }),
            ),
        ));
        routes.extend(api_docs);

        routes
    }
}

#[derive(Clone)]
struct ServeApiDoc(utoipa::openapi::OpenApi);

#[rocket::async_trait]
impl Handler for ServeApiDoc {
    async fn handle<'r>(&self, request: &'r Request<'_>, _: RocketData<'r>) -> Outcome<'r> {
        Outcome::from(request, Json(self.0.clone()))
    }
}

#[derive(Clone)]
struct ServeSwagger(Cow<'static, str>, Arc<Config<'static>>);

#[rocket::async_trait]
impl Handler for ServeSwagger {
    async fn handle<'r>(&self, request: &'r Request<'_>, _: RocketData<'r>) -> Outcome<'r> {
        let mut path = self.0.as_ref();
        if let Some(index) = self.0.find('<') {
            path = &path[..index];
        }

        match super::serve(&request.uri().path().as_str()[path.len()..], self.1.clone()) {
            Ok(swagger_file) => swagger_file
                .map(|file| Outcome::from(request, file))
                .unwrap_or_else(|| Outcome::from(request, NotFound("Swagger UI file not found"))),
            Err(error) => Outcome::from(
                request,
                status::Custom(Status::InternalServerError, error.to_string()),
            ),
        }
    }
}

impl<'r, 'o: 'r> RocketResponder<'r, 'o> for SwaggerFile<'o> {
    fn respond_to(self, _: &'r Request<'_>) -> rocket::response::Result<'o> {
        rocket::response::Result::Ok(
            Response::build()
                .header(Header::new("Content-Type", self.content_type))
                .sized_body(self.bytes.len(), Cursor::new(self.bytes.to_vec()))
                .status(Status::Ok)
                .finalize(),
        )
    }
}
