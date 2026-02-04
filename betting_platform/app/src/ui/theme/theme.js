"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.theme = void 0;
const tokens_1 = require("./tokens");
exports.theme = {
    colors: tokens_1.designTokens.colors,
    typography: tokens_1.designTokens.typography,
    spacing: tokens_1.designTokens.spacing,
    animation: tokens_1.designTokens.animation,
    breakpoints: tokens_1.designTokens.breakpoints,
    components: tokens_1.componentTokens
};
