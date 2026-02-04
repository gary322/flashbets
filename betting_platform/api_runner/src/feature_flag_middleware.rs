//! Feature flag middleware for protecting endpoints

use axum::{
    extract::State,
    http::Request,
    middleware::Next,
    response::{IntoResponse, Response},
    Extension,
};
use std::sync::Arc;
use tower::Layer;
use tracing::{debug, warn};

use crate::{
    AppState,
    feature_flags::{EvaluationContext, FeatureFlagService},
    jwt_validation::AuthenticatedUser,
    typed_errors::{AppError, ErrorKind, ErrorContext},
};

/// Feature flag middleware layer
#[derive(Clone)]
pub struct FeatureFlagLayer {
    flag_name: String,
}

impl FeatureFlagLayer {
    pub fn new(flag_name: impl Into<String>) -> Self {
        Self {
            flag_name: flag_name.into(),
        }
    }
}

impl<S> Layer<S> for FeatureFlagLayer {
    type Service = FeatureFlagMiddleware<S>;

    fn layer(&self, inner: S) -> Self::Service {
        FeatureFlagMiddleware {
            inner,
            flag_name: self.flag_name.clone(),
        }
    }
}

/// Feature flag middleware service
#[derive(Clone)]
pub struct FeatureFlagMiddleware<S> {
    inner: S,
    flag_name: String,
}

impl<S, B> tower::Service<Request<B>> for FeatureFlagMiddleware<S>
where
    S: tower::Service<Request<B>, Response = Response> + Clone + Send + 'static,
    S::Future: Send + 'static,
    B: Send + 'static,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<Self::Response, Self::Error>> + Send>
    >;

    fn poll_ready(
        &mut self,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: Request<B>) -> Self::Future {
        let flag_name = self.flag_name.clone();
        let inner = self.inner.clone();
        let mut inner = std::mem::replace(&mut self.inner, inner);

        Box::pin(async move {
            // Extract state
            let state = req.extensions()
                .get::<Arc<AppState>>()
                .cloned();
            
            if let Some(state) = state {
                if let Some(feature_service) = &state.feature_flags {
                    // Build evaluation context
                    let mut context = EvaluationContext::default();
                    
                    // Get user from extensions if authenticated
                    if let Some(user) = req.extensions().get::<AuthenticatedUser>() {
                        context.user_id = Some(user.claims.wallet.clone());
                    }
                    
                    // Get IP from headers
                    if let Some(ip) = req.headers()
                        .get("x-forwarded-for")
                        .and_then(|v| v.to_str().ok())
                        .map(|s| s.split(',').next().unwrap_or(s).trim().to_string())
                    {
                        context.ip_address = Some(ip);
                    }
                    
                    // Check if feature is enabled
                    match feature_service.is_enabled(&flag_name, &context).await {
                        Ok(true) => {
                            debug!("Feature flag '{}' is enabled", flag_name);
                            inner.call(req).await
                        }
                        Ok(false) => {
                            warn!("Feature flag '{}' is disabled", flag_name);
                            let error = AppError::new(
                                ErrorKind::FeatureDisabled,
                                format!("Feature '{}' is not enabled", flag_name),
                                ErrorContext::new("feature_flag_middleware", "check"),
                            );
                            Ok(error.into_response())
                        }
                        Err(e) => {
                            warn!("Failed to check feature flag '{}': {}", flag_name, e);
                            // Fail open - allow request to proceed
                            inner.call(req).await
                        }
                    }
                } else {
                    // No feature service - allow request
                    inner.call(req).await
                }
            } else {
                // No state - allow request
                inner.call(req).await
            }
        })
    }
}

/// Helper function to create feature flag protected routes
pub fn require_feature(flag_name: &str) -> FeatureFlagLayer {
    FeatureFlagLayer::new(flag_name)
}

/// Feature flag extractor for use in handlers
pub struct RequireFeature(pub String);

#[axum::async_trait]
impl<S> axum::extract::FromRequestParts<S> for RequireFeature
where
    S: Send + Sync,
{
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut axum::http::request::Parts,
        state: &S,
    ) -> Result<Self, Self::Rejection> {
        // Extract feature name from route extensions
        let feature_name = parts.extensions
            .get::<String>()
            .cloned()
            .unwrap_or_else(|| "unknown".to_string());
        
        // Extract app state
        let app_state = parts.extensions
            .get::<Arc<AppState>>()
            .ok_or_else(|| AppError::new(
                ErrorKind::InternalError,
                "App state not found",
                ErrorContext::new("feature_flag_middleware", "extract"),
            ))?;
        
        let feature_service = app_state.feature_flags.as_ref()
            .ok_or_else(|| AppError::new(
                ErrorKind::ServiceUnavailable,
                "Feature flag service not available",
                ErrorContext::new("feature_flag_middleware", "extract"),
            ))?;
        
        // Build evaluation context
        let mut context = EvaluationContext::default();
        
        // Get user if authenticated
        if let Some(user) = parts.extensions.get::<AuthenticatedUser>() {
            context.user_id = Some(user.claims.wallet.clone());
        }
        
        // Check feature
        if !feature_service.is_enabled(&feature_name, &context).await? {
            return Err(AppError::new(
                ErrorKind::FeatureDisabled,
                format!("Feature '{}' is not enabled", feature_name),
                ErrorContext::new("feature_flag_middleware", "extract"),
            ));
        }
        
        Ok(RequireFeature(feature_name))
    }
}

/// Conditional middleware that only applies if feature is enabled
pub async fn conditional_middleware<B>(
    State(state): State<Arc<AppState>>,
    Extension(user): Extension<Option<AuthenticatedUser>>,
    req: Request<B>,
    next: Next<B>,
    flag_name: &str,
) -> Result<Response, AppError> {
    let context = ErrorContext::new("feature_flag_middleware", "conditional");
    
    let feature_service = state.feature_flags.as_ref()
        .ok_or_else(|| AppError::new(
            ErrorKind::ServiceUnavailable,
            "Feature flag service not available",
            context.clone(),
        ))?;
    
    let mut eval_context = EvaluationContext::default();
    if let Some(user) = user {
        eval_context.user_id = Some(user.claims.wallet);
    }
    
    let enabled = feature_service.is_enabled(flag_name, &eval_context).await?;
    
    if enabled {
        // Apply some conditional logic here
        debug!("Applying conditional middleware for feature '{}'", flag_name);
    }
    
    Ok(next.run(req).await)
}