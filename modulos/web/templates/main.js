// @ts-check
"use strict";

const button = document.getElementById("hello");
const output = document.getElementById("output");

button?.addEventListener("click", () => {
  if (output) output.textContent = "Hello from {{name}}!";
});
