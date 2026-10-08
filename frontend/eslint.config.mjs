import eslintReact from "@eslint-react/eslint-plugin";
import js from "@eslint/js";
import prettier from "eslint-config-prettier";
import { defineConfig } from "eslint/config";
import globals from "globals";

export default defineConfig({
  files: ["src/**/*.{js,jsx}"],
  extends: [js.configs.recommended, eslintReact.configs.recommended, prettier],
  languageOptions: {
    ecmaVersion: "latest",
    sourceType: "module",
    parserOptions: {
      ecmaFeatures: { jsx: true },
    },
    globals: globals.browser,
  },
});
