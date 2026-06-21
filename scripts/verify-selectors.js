/**
 * Browser console script to verify form selectors
 *
 * Usage:
 * 1. Open a job board's application page in Chrome/Edge
 * 2. Open DevTools (F12) and go to Console
 * 3. Paste this script and run it
 * 4. Call the appropriate test function for the board
 */

const testSelectors = (board, selectors) => {
  console.log(`\n=== Testing ${board} selectors ===\n`);

  for (const [field, selectorList] of Object.entries(selectors)) {
    console.log(`\n${field}:`);

    for (const selector of selectorList) {
      try {
        const element = document.querySelector(selector);
        if (element) {
          console.log(`  ✓ Found: ${selector}`);
          console.log(`    Tag: ${element.tagName}, Type: ${element.type || 'N/A'}`);
        } else {
          console.log(`  ✗ Not found: ${selector}`);
        }
      } catch (e) {
        console.log(`  ✗ Error: ${selector} - ${e.message}`);
      }
    }
  }
};

const linkedinSelectors = {
  firstName: ["input[name='firstName']"],
  lastName: ["input[name='lastName']"],
  email: ["input[type='email']"],
  phone: ["input[type='tel']"],
  resume: ["input[type='file']", "[data-test='resume-upload']"],
  coverLetter: ["textarea[name='coverLetter']", "[data-test='cover-letter']"],
  submit: ["button[type='submit']", "[data-test='apply-submit']"],
  captcha: ["[data-test='captcha']", '.captcha'],
};

const greenhouseSelectors = {
  name: ["input[name='name']"],
  email: ["input[type='email']"],
  phone: ["input[type='tel']"],
  resume: ["input[type='file']", "[id='resume']"],
  coverLetter: ["textarea[name='cover_letter']", "[id='cover_letter']"],
  submit: ["button[type='submit']", "[id='submit-application']"],
  captcha: ['.g-recaptcha', '[data-sitekey]'],
};

// Test functions
const testLinkedIn = () => testSelectors('LinkedIn', linkedinSelectors);
const testGreenhouse = () => testSelectors('Greenhouse', greenhouseSelectors);

// Run all tests
const testAll = () => {
  testLinkedIn();
  testGreenhouse();
};

// Expose on window so the DevTools console can call them after pasting
// (also makes ESLint stop flagging them as unused — they ARE used as the
// public surface of this helper script).
if (typeof window !== 'undefined') {
  window.testLinkedIn = testLinkedIn;
  window.testGreenhouse = testGreenhouse;
  window.testAll = testAll;
}

console.log(
  'Selector verification script loaded. Call testLinkedIn(), testGreenhouse(), or testAll()'
);
