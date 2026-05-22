# LinkedIn Scraping Fix - Debugging & Resolution

## Problem

LinkedIn job scraping was not working in either guest mode or authenticated mode. No response was coming back from the scraper, making it impossible to search for jobs.

---

## Root Cause Analysis

### **Issue 1: No Error Logging**

The LinkedIn scraper had **zero error logging**, making it impossible to diagnose failures:

```rust
// Before: Silent failures
let html = self.client.get_html(&url, signal).await?;
// If this fails, user sees nothing
```

**Impact**: When LinkedIn blocks requests or changes their API, the scraper fails silently with no indication of what went wrong.

---

### **Issue 2: LinkedIn API Changes**

LinkedIn frequently changes their HTML structure and may block automated requests. Without logging, we can't see:

- HTTP status codes (403 Forbidden, 429 Rate Limited, etc.)
- Response content type (HTML vs JSON vs error page)
- Actual response body (to see if selectors still work)

---

### **Issue 3: No Request Visibility**

The scraper didn't log:

- Which URL is being requested
- Whether session cookies are being sent
- What the response looks like
- How many jobs were found

---

## Solution Implemented

### **1. Comprehensive Logging in HTTP Client**

Added detailed logging to `client.rs`:

```rust
pub async fn get_html(...) -> Result<String> {
    // Log request
    eprintln!("[LinkedIn] GET {}", url);
    eprintln!("[LinkedIn] Has session: {}", self.session_data.is_some());

    let response = self.client.get(url).headers(headers).send().await?;

    // Log response status and content type
    let status = response.status();
    let content_type = response.headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("unknown");

    eprintln!("[LinkedIn] Response: {} {}", status, content_type);

    // Log error responses
    if !status.is_success() {
        let error_body = response.text().await.unwrap_or_else(|_| String::from("<no body>"));
        eprintln!("[LinkedIn] Error response body (first 500 chars): {}",
            &error_body[..error_body.len().min(500)]);
        return Err(anyhow::anyhow!("HTTP {}: Request failed", status));
    }

    // Log response body info
    eprintln!("[LinkedIn] Response body length: {} bytes", body.len());
    eprintln!("[LinkedIn] Response preview (first 200 chars): {}",
        &body[..body.len().min(200)]);

    Ok(body)
}
```

**Benefits:**

- ✅ See exact URLs being requested
- ✅ Know if session cookies are being used
- ✅ See HTTP status codes
- ✅ See response content type
- ✅ See error messages from LinkedIn
- ✅ Preview response body to verify HTML structure

---

### **2. API Client Logging**

Added logging to `api_client.rs`:

```rust
pub async fn search_guest(...) -> Result<Vec<JobPosting>> {
    // Log search parameters
    eprintln!("[LinkedIn API] Searching with keywords: '{}', location: {:?}, start: {}",
        params.keywords, params.location, params.start);

    // Log URL
    eprintln!("[LinkedIn API] Request URL: {}", url);

    let html = self.client.get_html(&url, signal).await?;

    // Log parsing
    eprintln!("[LinkedIn API] Parsing HTML response...");
    let document = Html::parse_document(&html);

    // ... parsing logic ...

    // Log results
    eprintln!("[LinkedIn API] Found {} jobs on this page", jobs.len());
    Ok(jobs)
}
```

**Benefits:**

- ✅ See search parameters
- ✅ See full request URL with filters
- ✅ Know when parsing starts
- ✅ See how many jobs were found

---

## How to Diagnose Issues Now

### **Step 1: Run a LinkedIn Search**

In the app, try to scrape LinkedIn jobs. Watch the console output.

### **Step 2: Check the Logs**

You'll now see output like:

```
[LinkedIn API] Searching with keywords: 'software engineer', location: Some("Berlin"), start: 0
[LinkedIn API] Request URL: https://www.linkedin.com/jobs-guest/jobs/api/seeMoreJobPostings/search?keywords=software+engineer&start=0&location=Berlin
[LinkedIn] GET https://www.linkedin.com/jobs-guest/jobs/api/seeMoreJobPostings/search?keywords=software+engineer&start=0&location=Berlin
[LinkedIn] Has session: false
[LinkedIn] Response: 200 text/html
[LinkedIn] Response body length: 45231 bytes
[LinkedIn] Response preview (first 200 chars): <html><body><ul class="jobs-search__results-list">...
[LinkedIn API] Parsing HTML response...
[LinkedIn API] Found 25 jobs on this page
```

### **Step 3: Identify the Problem**

**If you see:**

**A) `Response: 403 Forbidden`**

- LinkedIn is blocking the request
- **Solution**: Add more realistic headers, use authenticated session, or add delays

**B) `Response: 429 Too Many Requests`**

- Rate limited
- **Solution**: Increase delays between requests in rate limiter

**C) `Response: 200` but `Found 0 jobs`**

- HTML structure changed
- **Solution**: Check response preview, update CSS selectors

**D) `Response: 302` or `Response: 301`**

- Redirect (possibly to login page)
- **Solution**: Follow redirects or use authenticated session

**E) No logs at all**

- Request never reached LinkedIn
- **Solution**: Check network connection, proxy settings

---

## Common LinkedIn Issues & Solutions

### **Issue: LinkedIn Blocks Guest API**

**Symptoms:**

```
[LinkedIn] Response: 403 text/html
[LinkedIn] Error response body: <html>...Access Denied...
```

**Solution:**

1. Use authenticated session (login via board_login)
2. Add more realistic headers
3. Use residential proxy

---

### **Issue: HTML Selectors Changed**

**Symptoms:**

```
[LinkedIn] Response: 200 text/html
[LinkedIn API] Found 0 jobs on this page
```

**Solution:**

1. Check response preview in logs
2. Update CSS selectors in `api_client.rs`:
   ```rust
   let link_selector = scraper::Selector::parse("a.base-card__full-link, a.base-search-card__link").unwrap();
   ```
3. Test with real LinkedIn HTML

---

### **Issue: Rate Limited**

**Symptoms:**

```
[LinkedIn] Response: 429 text/html
```

**Solution:**

1. Increase delay in `rate_limiter.rs`
2. Use authenticated session (higher limits)
3. Reduce concurrent requests

---

### **Issue: Session Expired**

**Symptoms:**

```
[LinkedIn] Has session: true
[LinkedIn] Response: 401 Unauthorized
```

**Solution:**

1. Re-login via board_login
2. Check session age (max 7 days)
3. Verify cookies are valid

---

## Testing Checklist

After this fix, test the following:

### **Guest Mode (No Authentication)**

- [ ] Search for jobs with keywords
- [ ] Search with location filter
- [ ] Search with date filter
- [ ] Paginate through results
- [ ] Check console logs for errors

### **Authenticated Mode**

- [ ] Login via board_login
- [ ] Verify session is saved
- [ ] Search for jobs
- [ ] Check "Has session: true" in logs
- [ ] Verify cookies are sent

### **Error Handling**

- [ ] Try invalid search (should show error)
- [ ] Try with network disconnected (should show error)
- [ ] Cancel mid-scrape (should abort cleanly)

---

## Next Steps

### **Immediate (Testing)**

1. Build the app: `rtk pnpm tauri build`
2. Run a LinkedIn search
3. Check console logs
4. Report findings

### **If Guest API is Blocked**

1. Implement authenticated API endpoint
2. Use different LinkedIn API (e.g., `/voyager/api/search/hits`)
3. Add proxy support

### **If Selectors Changed**

1. Inspect LinkedIn HTML
2. Update selectors in `api_client.rs`
3. Add fallback selectors

### **Long-Term**

1. Add automated tests with mock responses
2. Monitor LinkedIn API changes
3. Implement fallback strategies
4. Add retry logic with exponential backoff

---

## Files Modified

```
apps/tauri/src-tauri/src/scraping/linkedin/
├── client.rs           ✅ Added comprehensive logging
└── api_client.rs       ✅ Added search logging
```

---

## Summary

**Problem**: LinkedIn scraping failed silently with no error messages.

**Solution**: Added comprehensive logging to diagnose issues.

**Impact**:

- ✅ Can now see exactly what's happening
- ✅ Can diagnose LinkedIn API changes
- ✅ Can see rate limiting issues
- ✅ Can verify session authentication
- ✅ Can debug selector issues

**Next**: Build and test to see actual error messages, then fix the root cause.
