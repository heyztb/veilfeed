import { mount } from "svelte";
import App from "./App.svelte";
import { applyReaderFont, getReaderFont } from "./lib/readerFont";
import { applyTheme, getTheme } from "./lib/theme";
import "./styles.css";

applyTheme(getTheme());
applyReaderFont(getReaderFont());
mount(App, { target: document.getElementById("app")! });
