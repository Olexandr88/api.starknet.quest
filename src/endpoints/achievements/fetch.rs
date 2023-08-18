use std::sync::Arc;

use crate::{
    models::{AchievementCategoryDocument, AchievementQuery, AppState, UserAchievements},
    utils::get_error,
};
use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use futures::stream::StreamExt;
use mongodb::bson::{doc, from_document};
use starknet::core::types::FieldElement;

pub async fn handler(
    State(state): State<Arc<AppState>>,
    Query(query): Query<AchievementQuery>,
) -> impl IntoResponse {
    if query.addr == FieldElement::ZERO {
        return get_error("Please connect your wallet first".to_string());
    }
    let addr = FieldElement::to_string(&query.addr);
    let achievement_categories = state
        .db
        .collection::<AchievementCategoryDocument>("achievement_categories");
    let pipeline = vec![
        doc! {
          "$lookup": {
            "from": "achievements",
            "localField": "id",
            "foreignField": "category_id",
            "as": "achievement"
          }
        },
        doc! {"$unwind": "$achievement" },
        doc! {
          "$lookup": {
            "from": "achieved",
            "let": { "achievement_id": "$achievement.id" },
            "pipeline": [
                { "$match": {
                  "$expr": {
                    "$and": [
                      { "$eq": ["$achievement_id", "$$achievement_id"] },
                      { "$eq": ["$addr", addr] }
                    ]
                  }
                } }
              ],
              "as": "achieved"
          }
        },
        doc! {
          "$project": {
            "_id": 0,
            "category_name": "$name",
            "category_desc": "$desc",
            "achievements": {
              "name": "$achievement.name",
              "short_desc": "$achievement.short_desc",
              "title": {
                "$cond": [
                  { "$eq": [{ "$size": "$achieved" }, 0] },
                  "$achievement.todo_title",
                  "$achievement.done_title"
                ]
              },
              "desc": {
                "$cond": [
                  { "$eq": [{ "$size": "$achieved" }, 0] },
                  "$achievement.todo_desc",
                  "$achievement.done_desc"
                ]
              },
              "completed": { "$ne": [{ "$size": "$achieved" }, 0] },
              "verify_type": "$achievement.verify_type"
            }
          }
        },
        doc! {
          "$group": {
            "_id": { "category_name": "$category_name", "category_desc": "$category_desc" },
            "achievements": { "$push": "$achievements" }
          }
        },
        doc! {
          "$project": {
            "category_name": "$_id.category_name",
            "category_desc": "$_id.category_desc",
            "achievements": 1,
            "_id": 0
          }
        },
    ];

    match achievement_categories.aggregate(pipeline, None).await {
        Ok(mut cursor) => {
            let mut achievements: Vec<UserAchievements> = Vec::new();
            while let Some(result) = cursor.next().await {
                match result {
                    Ok(document) => {
                        if let Ok(achievement) = from_document::<UserAchievements>(document) {
                            achievements.push(achievement);
                        }
                    }
                    _ => continue,
                }
            }
            (StatusCode::OK, Json(achievements)).into_response()
        }
        Err(e) => get_error(format!("Error fetching user achievements: {}", e)),
    }
}
