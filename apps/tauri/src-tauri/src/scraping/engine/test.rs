use super::*;

#[test]
fn test_catalog() {
    let engine = ScraperEngine::new();
    let catalog = engine.catalog();
    assert_eq!(catalog.len(), 20);
    
    // Check specific scrapers
    assert!(catalog.iter().any(|s| s.id == "linkedin"));
    assert!(catalog.iter().any(|s| s.id == "indeed"));
    assert!(catalog.iter().any(|s| s.id == "ycombinator"));
}

#[test]
fn test_health() {
    let engine = ScraperEngine::new();
    let health = engine.health();
    assert_eq!(health.mode, "in-process");
    assert!(health.ready);
    assert_eq!(health.scrapers.len(), 20);
}

#[test]
fn test_performance_mode() {
    let engine = ScraperEngine::new();
    
    // Default mode should have semaphore limit of 2
    engine.set_performance_mode("default");
    
    // Low memory mode
    engine.set_performance_mode("low-memory");
    
    // Performance mode
    engine.set_performance_mode("performance");
    
    // Unknown mode should use default
    engine.set_performance_mode("unknown");
}

#[tokio::test]
async fn test_token_registration() {
    let engine = ScraperEngine::new();
    let token = tokio_util::sync::CancellationToken::new();
    
    engine.register_token("job-1", token.clone()).await;
    engine.unregister_token("job-1").await;
}

#[tokio::test]
async fn test_cancel_nonexistent_job() {
    let engine = ScraperEngine::new();
    // Should not panic for nonexistent job
    engine.cancel("nonexistent").await;
}

#[tokio::test]
async fn test_shutdown() {
    let engine = ScraperEngine::new();
    // Register some tokens
    let token1 = tokio_util::sync::CancellationToken::new();
    let token2 = tokio_util::sync::CancellationToken::new();
    
    engine.register_token("job-1", token1).await;
    engine.register_token("job-2", token2).await;
    
    engine.shutdown().await;
}
