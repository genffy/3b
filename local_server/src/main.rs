use std::path::PathBuf;
use std::sync::Arc;

use actix_cors::Cors;
use actix_web::{App, http, HttpServer, web};
use actix_web::web::Data;
use inference_core::{Document, init_semantic_with_path, InMemoryEmbeddingStore};
use uuid::Uuid;

use app_state::AppState;
use crate::doc_indexes::doc_indexes::DocIndexes;
use crate::doc_indexes::doc_schema::DocumentFile;

use crate::doc_split::split::split;
use crate::doc_split::splitter::SplitOptions;
use crate::document_handler::{create_embedding_document, search_embedding_document};
use crate::infra::file_walker::FileWalker;

pub mod scraper;
mod document_handler;
pub mod app_state;
pub mod infra;
pub mod doc_split;
pub mod doc_indexes;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let app_state = create_app_state();

    HttpServer::new(move || {
        App::new()
            .wrap(Cors::default()
                .allowed_origin("https://editor.unitmesh.cc")
                .allowed_methods(vec!["GET", "POST"])
                .allowed_headers(vec![http::header::AUTHORIZATION, http::header::ACCEPT])
                .allowed_header(http::header::CONTENT_TYPE)
                .max_age(3600)
            )
            .app_data(app_state.clone())
            .service(create_embedding_document)
            .service(search_embedding_document)
    })
        .bind(("127.0.0.1", 8080))?
        .run()
        .await
}

fn create_app_state() -> Data<AppState> {
    let semantic = init_semantic_with_path("model/model.onnx", "model/tokenizer.json")
        .expect("Failed to initialize semantic");

    let dir = PathBuf::from("testdocs");
    let files = FileWalker::index_directory(dir);

    let embedding_store = InMemoryEmbeddingStore::new();
    let options = SplitOptions::default();
    let indexes = DocIndexes::new();
    let mut index_writer = indexes.doc.writer(50_000_000).unwrap();

    files.iter().for_each(|file| {
        // DocumentFile::build_document(&file)?.map(|doc| {
        //     index_writer.add_document(doc).expect("TODO: panic message");
        // });

        if let Some(docs) = split(&file, &options) {
            docs.iter().for_each(|doc| {
                let embedding = semantic.embed(&doc.text).unwrap();
                let mut document = doc.clone();
                document.vector = embedding.clone();

                let uuid = Uuid::new_v4().to_string();
                embedding_store.add(uuid, embedding, document);
            });
        }
    });

    let app_state = web::Data::new(AppState {
        semantic,
        storage: Arc::new(embedding_store),
        indexes: Arc::new(indexes),
    });

    app_state
}