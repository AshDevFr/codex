//! Integration tests for user rating endpoints

#[path = "../common/mod.rs"]
mod common;

use codex::api::dto::series::{SetUserRatingRequest, UserRatingsListResponse, UserSeriesRatingDto};
use codex::api::error::ErrorResponse;
use codex::db::repositories::{LibraryRepository, SeriesRepository, UserRepository};
use codex::db::ScanningStrategy;
use codex::utils::password;
use common::*;
use hyper::StatusCode;

// Helper to create admin and token
async fn create_admin_and_token(
    db: &sea_orm::DatabaseConnection,
    state: &codex::api::extractors::AuthState,
) -> String {
    let password_hash = password::hash_password("admin123").unwrap();
    let user = create_test_user("admin", "admin@example.com", &password_hash, true);
    let created = UserRepository::create(db, &user).await.unwrap();
    state
        .jwt_service
        .generate_token(created.id, created.username, created.is_admin)
        .unwrap()
}

// ============================================================================
// Get Rating Tests
// ============================================================================

#[tokio::test]
async fn test_get_series_rating_not_found() {
    let (db, _temp_dir) = setup_test_db().await;

    let library = LibraryRepository::create(&db, "Library", "/lib", ScanningStrategy::Default)
        .await
        .unwrap();

    let series = SeriesRepository::create(&db, library.id, "Test Series", None)
        .await
        .unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    let request = get_request_with_auth(&format!("/api/v1/series/{}/rating", series.id), &token);
    let (status, _response): (StatusCode, Option<ErrorResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_get_series_rating_series_not_found() {
    let (db, _temp_dir) = setup_test_db().await;

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    let fake_id = uuid::Uuid::new_v4();
    let request = get_request_with_auth(&format!("/api/v1/series/{}/rating", fake_id), &token);
    let (status, _response): (StatusCode, Option<ErrorResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::NOT_FOUND);
}

// ============================================================================
// Set Rating Tests
// ============================================================================

#[tokio::test]
async fn test_set_series_rating_create() {
    let (db, _temp_dir) = setup_test_db().await;

    let library = LibraryRepository::create(&db, "Library", "/lib", ScanningStrategy::Default)
        .await
        .unwrap();

    let series = SeriesRepository::create(&db, library.id, "Test Series", None)
        .await
        .unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    // Create rating
    let body = SetUserRatingRequest {
        rating: 85,
        notes: Some("Great series!".to_string()),
    };
    let request = put_json_request_with_auth(
        &format!("/api/v1/series/{}/rating", series.id),
        &body,
        &token,
    );
    let (status, response): (StatusCode, Option<UserSeriesRatingDto>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let rating_response = response.unwrap();
    assert_eq!(rating_response.rating, 85);
    assert_eq!(rating_response.notes, Some("Great series!".to_string()));
    assert_eq!(rating_response.series_id, series.id);
}

#[tokio::test]
async fn test_set_series_rating_update() {
    let (db, _temp_dir) = setup_test_db().await;

    let library = LibraryRepository::create(&db, "Library", "/lib", ScanningStrategy::Default)
        .await
        .unwrap();

    let series = SeriesRepository::create(&db, library.id, "Test Series", None)
        .await
        .unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;

    // Create first rating
    {
        let app = create_test_router(state.clone()).await;
        let body = SetUserRatingRequest {
            rating: 60,
            notes: None,
        };
        let request = put_json_request_with_auth(
            &format!("/api/v1/series/{}/rating", series.id),
            &body,
            &token,
        );
        let (status, _): (StatusCode, Option<UserSeriesRatingDto>) =
            make_json_request(app, request).await;
        assert_eq!(status, StatusCode::OK);
    }

    // Update rating
    {
        let app = create_test_router(state.clone()).await;
        let body = SetUserRatingRequest {
            rating: 90,
            notes: Some("Changed my mind!".to_string()),
        };
        let request = put_json_request_with_auth(
            &format!("/api/v1/series/{}/rating", series.id),
            &body,
            &token,
        );
        let (status, response): (StatusCode, Option<UserSeriesRatingDto>) =
            make_json_request(app, request).await;

        assert_eq!(status, StatusCode::OK);
        let rating_response = response.unwrap();
        assert_eq!(rating_response.rating, 90);
        assert_eq!(rating_response.notes, Some("Changed my mind!".to_string()));
    }
}

#[tokio::test]
async fn test_set_series_rating_invalid_range_low() {
    let (db, _temp_dir) = setup_test_db().await;

    let library = LibraryRepository::create(&db, "Library", "/lib", ScanningStrategy::Default)
        .await
        .unwrap();

    let series = SeriesRepository::create(&db, library.id, "Test Series", None)
        .await
        .unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    let body = SetUserRatingRequest {
        rating: 0,
        notes: None,
    };
    let request = put_json_request_with_auth(
        &format!("/api/v1/series/{}/rating", series.id),
        &body,
        &token,
    );
    let (status, _response): (StatusCode, Option<ErrorResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_set_series_rating_invalid_range_high() {
    let (db, _temp_dir) = setup_test_db().await;

    let library = LibraryRepository::create(&db, "Library", "/lib", ScanningStrategy::Default)
        .await
        .unwrap();

    let series = SeriesRepository::create(&db, library.id, "Test Series", None)
        .await
        .unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    let body = SetUserRatingRequest {
        rating: 101,
        notes: None,
    };
    let request = put_json_request_with_auth(
        &format!("/api/v1/series/{}/rating", series.id),
        &body,
        &token,
    );
    let (status, _response): (StatusCode, Option<ErrorResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
}

// ============================================================================
// Delete Rating Tests
// ============================================================================

#[tokio::test]
async fn test_delete_series_rating() {
    let (db, _temp_dir) = setup_test_db().await;

    let library = LibraryRepository::create(&db, "Library", "/lib", ScanningStrategy::Default)
        .await
        .unwrap();

    let series = SeriesRepository::create(&db, library.id, "Test Series", None)
        .await
        .unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;

    // Create rating first
    {
        let app = create_test_router(state.clone()).await;
        let body = SetUserRatingRequest {
            rating: 75,
            notes: None,
        };
        let request = put_json_request_with_auth(
            &format!("/api/v1/series/{}/rating", series.id),
            &body,
            &token,
        );
        let (status, _): (StatusCode, Option<UserSeriesRatingDto>) =
            make_json_request(app, request).await;
        assert_eq!(status, StatusCode::OK);
    }

    // Delete rating
    {
        let app = create_test_router(state.clone()).await;
        let request =
            delete_request_with_auth(&format!("/api/v1/series/{}/rating", series.id), &token);
        let (status, _) = make_request(app, request).await;
        assert_eq!(status, StatusCode::NO_CONTENT);
    }

    // Verify rating is gone
    {
        let app = create_test_router(state.clone()).await;
        let request =
            get_request_with_auth(&format!("/api/v1/series/{}/rating", series.id), &token);
        let (status, _response): (StatusCode, Option<ErrorResponse>) =
            make_json_request(app, request).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
    }
}

#[tokio::test]
async fn test_delete_series_rating_not_found() {
    let (db, _temp_dir) = setup_test_db().await;

    let library = LibraryRepository::create(&db, "Library", "/lib", ScanningStrategy::Default)
        .await
        .unwrap();

    let series = SeriesRepository::create(&db, library.id, "Test Series", None)
        .await
        .unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    let request = delete_request_with_auth(&format!("/api/v1/series/{}/rating", series.id), &token);
    let (status, _) = make_request(app, request).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

// ============================================================================
// List User Ratings Tests
// ============================================================================

#[tokio::test]
async fn test_list_user_ratings_empty() {
    let (db, _temp_dir) = setup_test_db().await;

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    let request = get_request_with_auth("/api/v1/user/ratings", &token);
    let (status, response): (StatusCode, Option<UserRatingsListResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let rating_response = response.unwrap();
    assert_eq!(rating_response.ratings.len(), 0);
}

#[tokio::test]
async fn test_list_user_ratings_with_data() {
    let (db, _temp_dir) = setup_test_db().await;

    let library = LibraryRepository::create(&db, "Library", "/lib", ScanningStrategy::Default)
        .await
        .unwrap();

    let series1 = SeriesRepository::create(&db, library.id, "Series 1", None)
        .await
        .unwrap();
    let series2 = SeriesRepository::create(&db, library.id, "Series 2", None)
        .await
        .unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;

    // Create ratings
    {
        let app = create_test_router(state.clone()).await;
        let body = SetUserRatingRequest {
            rating: 80,
            notes: None,
        };
        let request = put_json_request_with_auth(
            &format!("/api/v1/series/{}/rating", series1.id),
            &body,
            &token,
        );
        let (status, _): (StatusCode, Option<UserSeriesRatingDto>) =
            make_json_request(app, request).await;
        assert_eq!(status, StatusCode::OK);
    }
    {
        let app = create_test_router(state.clone()).await;
        let body = SetUserRatingRequest {
            rating: 90,
            notes: None,
        };
        let request = put_json_request_with_auth(
            &format!("/api/v1/series/{}/rating", series2.id),
            &body,
            &token,
        );
        let (status, _): (StatusCode, Option<UserSeriesRatingDto>) =
            make_json_request(app, request).await;
        assert_eq!(status, StatusCode::OK);
    }

    // List ratings
    let app = create_test_router(state.clone()).await;
    let request = get_request_with_auth("/api/v1/user/ratings", &token);
    let (status, response): (StatusCode, Option<UserRatingsListResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let rating_response = response.unwrap();
    assert_eq!(rating_response.ratings.len(), 2);
}

#[tokio::test]
async fn test_list_user_ratings_unauthorized() {
    let (db, _temp_dir) = setup_test_db().await;

    let state = create_test_auth_state(db.clone()).await;
    let app = create_test_router(state).await;

    // No auth token
    let request = get_request("/api/v1/user/ratings");
    let (status, _response): (StatusCode, Option<ErrorResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

// ============================================================================
// Average Rating Tests
// ============================================================================

use codex::api::dto::series::SeriesAverageRatingResponse;

#[tokio::test]
async fn test_get_series_average_rating_no_ratings() {
    let (db, _temp_dir) = setup_test_db().await;

    let library = LibraryRepository::create(&db, "Library", "/lib", ScanningStrategy::Default)
        .await
        .unwrap();

    let series = SeriesRepository::create(&db, library.id, "Test Series", None)
        .await
        .unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    let request = get_request_with_auth(
        &format!("/api/v1/series/{}/ratings/average", series.id),
        &token,
    );
    let (status, response): (StatusCode, Option<SeriesAverageRatingResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let avg_response = response.unwrap();
    assert!(avg_response.average.is_none());
    assert_eq!(avg_response.count, 0);
}

#[tokio::test]
async fn test_get_series_average_rating_single_rating() {
    let (db, _temp_dir) = setup_test_db().await;

    let library = LibraryRepository::create(&db, "Library", "/lib", ScanningStrategy::Default)
        .await
        .unwrap();

    let series = SeriesRepository::create(&db, library.id, "Test Series", None)
        .await
        .unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;

    // Create a rating first
    {
        let app = create_test_router(state.clone()).await;
        let body = SetUserRatingRequest {
            rating: 80,
            notes: None,
        };
        let request = put_json_request_with_auth(
            &format!("/api/v1/series/{}/rating", series.id),
            &body,
            &token,
        );
        let (status, _): (StatusCode, Option<UserSeriesRatingDto>) =
            make_json_request(app, request).await;
        assert_eq!(status, StatusCode::OK);
    }

    // Get average
    let app = create_test_router(state.clone()).await;
    let request = get_request_with_auth(
        &format!("/api/v1/series/{}/ratings/average", series.id),
        &token,
    );
    let (status, response): (StatusCode, Option<SeriesAverageRatingResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let avg_response = response.unwrap();
    assert_eq!(avg_response.average, Some(80.0));
    assert_eq!(avg_response.count, 1);
}

#[tokio::test]
async fn test_get_series_average_rating_multiple_users() {
    let (db, _temp_dir) = setup_test_db().await;

    let library = LibraryRepository::create(&db, "Library", "/lib", ScanningStrategy::Default)
        .await
        .unwrap();

    let series = SeriesRepository::create(&db, library.id, "Test Series", None)
        .await
        .unwrap();

    let state = create_test_auth_state(db.clone()).await;

    // Create first admin user and their rating
    let password_hash = password::hash_password("admin1pass").unwrap();
    let user1 = create_test_user("admin1", "admin1@example.com", &password_hash, true);
    let created_user1 = UserRepository::create(&db, &user1).await.unwrap();
    let token1 = state
        .jwt_service
        .generate_token(
            created_user1.id,
            created_user1.username,
            created_user1.is_admin,
        )
        .unwrap();

    // Create second admin user and their rating
    let password_hash2 = password::hash_password("admin2pass").unwrap();
    let user2 = create_test_user("admin2", "admin2@example.com", &password_hash2, true);
    let created_user2 = UserRepository::create(&db, &user2).await.unwrap();
    let token2 = state
        .jwt_service
        .generate_token(
            created_user2.id,
            created_user2.username,
            created_user2.is_admin,
        )
        .unwrap();

    // User 1 rates 70
    {
        let app = create_test_router(state.clone()).await;
        let body = SetUserRatingRequest {
            rating: 70,
            notes: None,
        };
        let request = put_json_request_with_auth(
            &format!("/api/v1/series/{}/rating", series.id),
            &body,
            &token1,
        );
        let (status, _): (StatusCode, Option<UserSeriesRatingDto>) =
            make_json_request(app, request).await;
        assert_eq!(status, StatusCode::OK);
    }

    // User 2 rates 90
    {
        let app = create_test_router(state.clone()).await;
        let body = SetUserRatingRequest {
            rating: 90,
            notes: None,
        };
        let request = put_json_request_with_auth(
            &format!("/api/v1/series/{}/rating", series.id),
            &body,
            &token2,
        );
        let (status, _): (StatusCode, Option<UserSeriesRatingDto>) =
            make_json_request(app, request).await;
        assert_eq!(status, StatusCode::OK);
    }

    // Get average - should be (70 + 90) / 2 = 80
    let app = create_test_router(state.clone()).await;
    let request = get_request_with_auth(
        &format!("/api/v1/series/{}/ratings/average", series.id),
        &token1,
    );
    let (status, response): (StatusCode, Option<SeriesAverageRatingResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let avg_response = response.unwrap();
    assert_eq!(avg_response.average, Some(80.0));
    assert_eq!(avg_response.count, 2);
}

#[tokio::test]
async fn test_get_series_average_rating_series_not_found() {
    let (db, _temp_dir) = setup_test_db().await;

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    let fake_id = uuid::Uuid::new_v4();
    let request = get_request_with_auth(
        &format!("/api/v1/series/{}/ratings/average", fake_id),
        &token,
    );
    let (status, _response): (StatusCode, Option<ErrorResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_get_series_average_rating_unauthorized() {
    let (db, _temp_dir) = setup_test_db().await;

    let library = LibraryRepository::create(&db, "Library", "/lib", ScanningStrategy::Default)
        .await
        .unwrap();

    let series = SeriesRepository::create(&db, library.id, "Test Series", None)
        .await
        .unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let app = create_test_router(state).await;

    // No auth token
    let request = get_request(&format!("/api/v1/series/{}/ratings/average", series.id));
    let (status, _response): (StatusCode, Option<ErrorResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::UNAUTHORIZED);
}
