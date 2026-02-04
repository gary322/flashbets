"use strict";
var __createBinding = (this && this.__createBinding) || (Object.create ? (function(o, m, k, k2) {
    if (k2 === undefined) k2 = k;
    var desc = Object.getOwnPropertyDescriptor(m, k);
    if (!desc || ("get" in desc ? !m.__esModule : desc.writable || desc.configurable)) {
      desc = { enumerable: true, get: function() { return m[k]; } };
    }
    Object.defineProperty(o, k2, desc);
}) : (function(o, m, k, k2) {
    if (k2 === undefined) k2 = k;
    o[k2] = m[k];
}));
var __exportStar = (this && this.__exportStar) || function(m, exports) {
    for (var p in m) if (p !== "default" && !Object.prototype.hasOwnProperty.call(exports, p)) __createBinding(exports, m, p);
};
var __importDefault = (this && this.__importDefault) || function (mod) {
    return (mod && mod.__esModule) ? mod : { "default": mod };
};
Object.defineProperty(exports, "__esModule", { value: true });
exports.render = void 0;
const react_1 = __importDefault(require("react"));
const react_2 = require("@testing-library/react");
const react_3 = require("@emotion/react");
const theme_1 = require("../theme");
const AllTheProviders = ({ children }) => {
    return (<react_3.ThemeProvider theme={theme_1.theme}>
      {children}
    </react_3.ThemeProvider>);
};
const customRender = (ui, options) => (0, react_2.render)(ui, Object.assign({ wrapper: AllTheProviders }, options));
exports.render = customRender;
__exportStar(require("@testing-library/react"), exports);
