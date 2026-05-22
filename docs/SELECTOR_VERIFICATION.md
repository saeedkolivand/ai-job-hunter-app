# Form Selector Verification Guide

This guide explains how to verify the form selectors for each board's application forms against their production UI.

## How to Verify Selectors

1. **Open the board's application page** in a browser (Chrome/Edge recommended)
2. **Open DevTools** (F12) and go to the Elements tab
3. **Use the element picker** (Ctrl+Shift+C) to select form fields
4. **Copy the selector** and compare with our mappings below

## Current Selector Mappings

### LinkedIn Easy Apply

**Selectors to verify:**

- Name: `input[name='firstName']`, `input[name='lastName']`
- Email: `input[type='email']`
- Phone: `input[type='tel']`
- Resume upload: `input[type='file']`, `[data-test='resume-upload']`
- Cover letter: `textarea[name='coverLetter']`, `[data-test='cover-letter']`
- Submit button: `button[type='submit']`, `[data-test='apply-submit']`
- CAPTCHA: `[data-test='captcha']`, `.captcha`

**Verification URL:**

- Navigate to any LinkedIn job posting
- Click "Easy Apply" button
- Inspect the form that appears

### Indeed Easy Apply

**Selectors to verify:**

- Name: `input[name='applicant.name']`
- Email: `input[type='email']`
- Phone: `input[type='tel']`
- Resume upload: `input[type='file']`, `[id='resume-upload']`
- Cover letter: `textarea[name='coverLetter']`, `[id='cover-letter']`
- Submit button: `button[type='submit']`, `[id='apply-button']`
- CAPTCHA: `.recaptcha`, `[data-captcha]`

**Verification URL:**

- Navigate to any Indeed job posting
- Click "Apply Now" or "Easy Apply"
- Inspect the application form

### Greenhouse

**Selectors to verify:**

- Name: `input[name='name']`
- Email: `input[type='email']`
- Phone: `input[type='tel']`
- Resume upload: `input[type='file']`, `[id='resume']`
- Cover letter: `textarea[name='cover_letter']`, `[id='cover_letter']`
- Submit button: `button[type='submit']`, `[id='submit-application']`
- CAPTCHA: `.g-recaptcha`, `[data-sitekey]`

**Verification URL:**

- Navigate to any company's Greenhouse job board (e.g., stripe.com/jobs)
- Click on a job posting
- Inspect the application form

### Workday

**Selectors to verify:**

- Name: `input[name='candidate-name']`
- Email: `input[type='email']`
- Phone: `input[type='tel']`
- Resume upload: `input[type='file']`, `[data-automation-id='fileUpload']`
- Cover letter: `textarea[name='cover-letter']`, `[data-automation-id='coverLetter']`
- Submit button: `button[type='submit']`, `[data-automation-id='submit']`
- CAPTCHA: `.captcha`, `[data-captcha]`

**Verification URL:**

- Navigate to any company's Workday job board (e.g., careers.jpmorgan.com)
- Click on a job posting
- Inspect the application form

## Testing Script

Use the following JavaScript in browser DevTools Console to test selectors:

```javascript
// Test LinkedIn selectors
const testLinkedIn = () => {
  const selectors = {
    firstName: "input[name='firstName']",
    lastName: "input[name='lastName']",
    email: "input[type='email']",
    phone: "input[type='tel']",
    resume: "input[type='file']",
    coverLetter: "textarea[name='coverLetter']",
    submit: "button[type='submit']",
  };

  for (const [field, selector] of Object.entries(selectors)) {
    const element = document.querySelector(selector);
    console.log(`${field}: ${element ? '✓ Found' : '✗ Not found'}`, selector);
  }
};

// Run the test
testLinkedIn();
```

## Updating Selectors

If selectors don't match:

1. Copy the correct selector from DevTools
2. Update `apps/tauri/src-tauri/src/applying/selectors.rs`
3. Run `cargo check` to verify the build
4. Test with the updated selectors

## Common Issues

- **Dynamic IDs**: If elements have random IDs, use stable attributes like `name`, `type`, or `data-*` attributes
- **Shadow DOM**: Some forms use shadow DOM - may need custom handling
- **iframe content**: Some forms load in iframes - need to switch context
- **JavaScript-rendered**: Wait for elements to appear before selecting

## Notes

- Selectors should be as specific as possible to avoid false positives
- Prefer `data-*` attributes over class names (more stable)
- Test multiple job postings to ensure consistency
- Document any board-specific quirks or variations
