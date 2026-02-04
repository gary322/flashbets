"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
const test_1 = require("@playwright/test");
exports.default = (0, test_1.defineConfig)({
    testDir: './tests',
    fullyParallel: true,
    forbidOnly: !!process.env.CI,
    retries: process.env.CI ? 2 : 0,
    workers: process.env.CI ? 1 : 4,
    reporter: [
        ['html'],
        ['junit', { outputFile: 'results.xml' }],
        ['json', { outputFile: 'results.json' }],
        ['list'],
    ],
    use: {
        baseURL: 'http://localhost:8080',
        trace: 'on-first-retry',
        screenshot: 'only-on-failure',
        video: 'retain-on-failure',
        // Global timeout
        actionTimeout: 30000,
        navigationTimeout: 30000,
    },
    projects: [
        {
            name: 'chromium',
            use: Object.assign({}, test_1.devices['Desktop Chrome']),
        },
        {
            name: 'firefox',
            use: Object.assign({}, test_1.devices['Desktop Firefox']),
        },
        {
            name: 'webkit',
            use: Object.assign({}, test_1.devices['Desktop Safari']),
        },
        // Mobile testing
        {
            name: 'Mobile Chrome',
            use: Object.assign({}, test_1.devices['Pixel 5']),
        },
        {
            name: 'Mobile Safari',
            use: Object.assign({}, test_1.devices['iPhone 12']),
        },
    ],
    // Run your local dev server before starting the tests
    webServer: [
        {
            command: 'cd ../../api_runner && cargo run',
            port: 8080,
            timeout: 120 * 1000,
            reuseExistingServer: !process.env.CI,
        },
        {
            command: 'cd ../../programs/betting_platform_native/ui_demo && node server.js',
            port: 8080,
            timeout: 120 * 1000,
            reuseExistingServer: !process.env.CI,
        }
    ],
});
