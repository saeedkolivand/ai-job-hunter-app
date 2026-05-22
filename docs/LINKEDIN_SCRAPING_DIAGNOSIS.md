# LinkedIn Scraping - Diagnosis & Fix Plan

## Current Status

**Problem**: LinkedIn job scraping returns no results in both guest mode and authenticated mode.

**Logging Added**: Comprehensive logging has been added to track every step of the scraping process.

---

## Diagnosis Plan

### **Step 1: Build & Test** ⏳ In Progress

1. **Build the app** with new logging:

   ```bash
   rtk pnpm tauri build
   ```

2. **Run the app** and try LinkedIn scraping

3. **Collect console logs** - Look for:
   ```
   [LinkedIn API] Searching with keywords: '...'
   [LinkedIn] GET https://www.linkedin.com/...
   [LinkedIn] Response: <STATUS> <CONTENT_TYPE>
   [LinkedIn] Response body length: <SIZE> bytes
   [LinkedIn API] Found <N> jobs on this page
   ```

---

### **Step 2: Analyze Logs** 📊

Based on the logs, we'll identify the issue:

#### **Scenario A: HTTP 403 Forbidden**

```
[LinkedIn] Response: 403 text/html
[LinkedIn] Error response body: Access Denied...
```

**Cause**: LinkedIn is blocking the request (bot detection)

**Fix Options**:

1. Improve headers to look more like a real browser
2. Add referrer and origin headers
3. Use authenticated session
4. Add random delays
5. Rotate user agents

---

#### **Scenario B: HTTP 429 Rate Limited**

```
[LinkedIn] Response: 429 text/html
```

**Cause**: Too many requests

**Fix Options**:

1. Increase rate limiter delays
2. Add exponential backoff
3. Reduce concurrent requests
4. Use authenticated session (higher limits)

---

#### **Scenario C: HTTP 200 but 0 Jobs**

```
[LinkedIn] Response: 200 text/html
[LinkedIn] Response body length: 45000 bytes
[LinkedIn API] Found 0 jobs on this page
```

**Cause**: HTML structure changed, selectors don't match

**Fix Options**:

1. Inspect response preview in logs
2. Update CSS selectors
3. Add fallback selectors
4. Switch to different API endpoint

---

#### **Scenario D: Redirect to Login**

```
[LinkedIn] Response: 302 text/html
[LinkedIn] Response preview: <html>...login...
```

**Cause**: LinkedIn requires authentication

**Fix Options**:

1. Use authenticated session
2. Handle redirects properly
3. Switch to authenticated API endpoint

---

#### **Scenario E: Network Error**

```
Error: Failed to fetch
```

**Cause**: Network/proxy issue

**Fix Options**:

1. Check internet connection
2. Check proxy settings
3. Add retry logic
4. Increase timeout

---

### **Step 3: Implement Fix** 🔧

Based on the diagnosis, we'll implement the appropriate fix.

---

## Likely Issues & Fixes

### **Most Likely: LinkedIn Blocking Guest API**

LinkedIn has been increasingly aggressive about blocking automated access. The guest API (`/jobs-guest/jobs/api/seeMoreJobPostings/search`) may be:

- Blocked entirely
- Requiring CAPTCHA
- Requiring authentication
- Rate limited aggressively

**Quick Fixes to Try**:

#### **Fix 1: Improve Headers**

```rust
// Add more realistic headers
headers.insert("Referer", "https://www.linkedin.com/jobs/search/");
headers.insert("Origin", "https://www.linkedin.com");
headers.insert("Sec-Ch-Ua", "\"Chromium\";v=\"124\", \"Not(A:Brand\";v=\"99\"");
headers.insert("Sec-Ch-Ua-Mobile", "?0");
headers.insert("Sec-Ch-Ua-Platform", "\"Windows\"");
```

#### **Fix 2: Use Authenticated Endpoint**

```rust
// Switch from guest API to authenticated API
let url = if has_session {
    "https://www.linkedin.com/voyager/api/search/hits"
} else {
    "https://www.linkedin.com/jobs-guest/jobs/api/seeMoreJobPostings/search"
};
```

#### **Fix 3: Add Delays**

```rust
// Add random delay before request
tokio::time::sleep(Duration::from_millis(1000 + rand::random::<u64>() % 2000)).await;
```

#### **Fix 4: Fallback to Browser Mode**

```rust
// If HTTP fails, fall back to browser scraping
if jobs.is_empty() && !used_browser {
    eprintln!("[LinkedIn] HTTP scraping failed, falling back to browser...");
    return browser_scrape(input, ctx).await;
}
```

---

## Testing Checklist

Once fix is implemented:

### **Guest Mode**

- [ ] Search with keywords only
- [ ] Search with keywords + location
- [ ] Search with date filter
- [ ] Paginate through results
- [ ] Verify job data (title, company, location)

### **Authenticated Mode**

- [ ] Login via board_login
- [ ] Verify session saved
- [ ] Search with session
- [ ] Check "Has session: true" in logs
- [ ] Verify more results than guest mode

### **Error Handling**

- [ ] Invalid search (should show error)
- [ ] Network disconnected (should show error)
- [ ] Cancel mid-scrape (should abort cleanly)

---

## Alternative Solutions

If the guest API is completely blocked:

### **Option 1: Use LinkedIn Authenticated API**

- Requires user login
- Higher rate limits
- Access to more job data
- More stable

### **Option 2: Use LinkedIn RSS Feed**

```
https://www.linkedin.com/jobs-guest/jobs/api/seeMoreJobPostings/search?keywords=...&f_TPR=r86400&output=RSS
```

- May still work
- Limited data
- No pagination

### **Option 3: Browser-Based Scraping**

- Use Chromium to load pages
- Slower but more reliable
- Harder to block
- Higher resource usage

### **Option 4: Use LinkedIn Official API**

- Requires API key
- Limited free tier
- Most stable
- Best data quality

---

## Next Steps

1. **Wait for build to complete** ⏳
2. **Run the app and test LinkedIn scraping**
3. **Share the console logs** with me
4. **I'll identify the exact issue**
5. **Implement the fix**
6. **Test and verify**

---

## Expected Timeline

- **Build**: ~5-10 minutes
- **Testing**: ~5 minutes
- **Diagnosis**: ~5 minutes (based on logs)
- **Fix Implementation**: ~15-30 minutes
- **Testing Fix**: ~10 minutes

**Total: ~40-60 minutes to resolution**

---

**Ready to diagnose once the build completes!** 🔍
