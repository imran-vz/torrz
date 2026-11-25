// Builds the static site into site/public: the landing page and 404 from site/src, and one
// docs page for each Markdown file in docs/. Run it after changing either, then commit
// site/public; Cloudflare serves those files as they are.
//
//   cd site && npm install && npm run build
//
// SITE_URL sets the address used in the sitemap and link previews.

import { readFileSync, writeFileSync, mkdirSync, rmSync, existsSync } from "node:fs";
import { dirname, join, posix } from "node:path";
import { fileURLToPath } from "node:url";
import { Marked } from "marked";

const here = dirname(fileURLToPath(import.meta.url));
const repo = join(here, "..");
const out = join(here, "public");
const SITE_URL = (process.env.SITE_URL || "https://torrz.imran.codes").replace(/\/$/, "");
const GITHUB = "https://github.com/imran-vz/torrz";
const BLOB = `${GITHUB}/blob/main`;

// The docs, in reading order. `slug` is the page's path under /docs/.
const PAGES = [
  { slug: "", file: "getting-started.md", nav: "Getting started", group: "Guide" },
  { slug: "cli", file: "cli.md", nav: "CLI reference", group: "Reference" },
  { slug: "architecture", file: "architecture.md", nav: "Architecture", group: "Reference" },
  { slug: "development", file: "development.md", nav: "Development", group: "Contributing" },
];
const EXTERNAL = [
  { href: `${BLOB}/CONTRIBUTING.md`, nav: "Contributing guide", group: "Contributing" },
  { href: `${BLOB}/CHANGELOG.md`, nav: "Changelog", group: "Contributing" },
];

const esc = (s) => String(s).replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;").replace(/"/g, "&quot;");
const pageUrl = (slug) => (slug ? `/docs/${slug}/` : "/docs/");

/** GitHub's heading anchors: lower case, punctuation dropped, spaces to hyphens. */
function slugify(text) {
  return text
    .toLowerCase()
    .replace(/<[^>]+>/g, "")
    .replace(/&[a-z]+;|&#\d+;/g, "")
    .replace(/[^\p{L}\p{N}\s_-]/gu, "")
    .trim()
    .replace(/\s/g, "-");
}

/** A link written for GitHub (docs/cli.md, ../LICENSE) as it should go on the site. */
function siteHref(href) {
  if (/^([a-z]+:|#|\/)/i.test(href)) return href;
  const [path, hash] = href.split("#");
  const anchor = hash ? `#${hash}` : "";
  const base = posix.basename(path);
  const page = PAGES.find((p) => p.file === base);
  if (page) return pageUrl(page.slug) + anchor;
  // Anything else in the repo is shown on GitHub, resolved from docs/.
  const resolved = posix.normalize(posix.join("docs", path));
  return `${BLOB}/${resolved}${anchor}`;
}

function render(markdown) {
  const seen = new Map();
  const headings = [];
  let title = "";
  const marked = new Marked({
    gfm: true,
    renderer: {
      heading({ tokens, depth }) {
        const html = this.parser.parseInline(tokens);
        if (depth === 1) {
          title = html.replace(/<[^>]+>/g, "");
          return `<h1>${html}</h1>\n`;
        }
        let id = slugify(html);
        const n = seen.get(id) || 0;
        seen.set(id, n + 1);
        if (n) id = `${id}-${n}`;
        if (depth <= 3) headings.push({ depth, id, html });
        return `<h${depth} id="${id}">${html}<a class="anchor" href="#${id}" aria-label="Link to this section">#</a></h${depth}>\n`;
      },
      link({ href, title: t, tokens }) {
        const text = this.parser.parseInline(tokens);
        const to = siteHref(href);
        const outside = /^https?:/.test(to) && !to.startsWith(SITE_URL);
        return `<a href="${esc(to)}"${t ? ` title="${esc(t)}"` : ""}${outside ? ' rel="noopener"' : ""}>${text}</a>`;
      },
    },
  });
  const body = marked
    .parse(markdown)
    .replace(/<table>/g, '<div class="table"><table>')
    .replace(/<\/table>/g, "</table></div>");
  return { title, body, headings };
}

const icons = {
  github: '<svg viewBox="0 0 16 16" fill="currentColor" aria-hidden="true"><path d="M8 0C3.58 0 0 3.58 0 8c0 3.54 2.29 6.53 5.47 7.59.4.07.55-.17.55-.38 0-.19-.01-.82-.01-1.49-2.01.37-2.53-.49-2.69-.94-.09-.23-.48-.94-.82-1.13-.28-.15-.68-.52-.01-.53.63-.01 1.08.58 1.23.82.72 1.21 1.87.87 2.33.66.07-.52.28-.87.51-1.07-1.78-.2-3.64-.89-3.64-3.95 0-.87.31-1.59.82-2.15-.08-.2-.36-1.02.08-2.12 0 0 .67-.21 2.2.82.64-.18 1.32-.27 2-.27.68 0 1.36.09 2 .27 1.53-1.04 2.2-.82 2.2-.82.44 1.1.16 1.92.08 2.12.51.56.82 1.27.82 2.15 0 3.07-1.87 3.75-3.65 3.95.29.25.54.73.54 1.48 0 1.07-.01 1.93-.01 2.2 0 .21.15.46.55.38A8.013 8.013 0 0016 8c0-4.42-3.58-8-8-8z"/></svg>',
  sun: '<svg class="sun" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" aria-hidden="true"><circle cx="12" cy="12" r="4"/><path d="M12 2v2M12 20v2M4.9 4.9l1.4 1.4M17.7 17.7l1.4 1.4M2 12h2M20 12h2M4.9 19.1l1.4-1.4M17.7 6.3l1.4-1.4"/></svg>',
  menu: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" aria-hidden="true"><path d="M4 7h16M4 12h16M4 17h16"/></svg>',
  moon: '<svg class="moon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M20.5 14.5A8.5 8.5 0 019.5 3.5a8.5 8.5 0 1011 11z"/></svg>',
};

function head({ title, description, path }) {
  const url = SITE_URL + path;
  return `<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>${esc(title)}</title>
<meta name="description" content="${esc(description)}">
<link rel="canonical" href="${url}">
<meta name="theme-color" content="#ffffff" media="(prefers-color-scheme: light)">
<meta name="theme-color" content="#111113" media="(prefers-color-scheme: dark)">
<meta property="og:type" content="website">
<meta property="og:site_name" content="Torrz">
<meta property="og:title" content="${esc(title)}">
<meta property="og:description" content="${esc(description)}">
<meta property="og:url" content="${url}">
<meta property="og:image" content="${SITE_URL}/og.png">
<meta property="og:image:width" content="1200">
<meta property="og:image:height" content="630">
<meta property="og:image:alt" content="Torrz: every click is a command. An open-source BitTorrent client for macOS.">
<meta name="twitter:card" content="summary_large_image">
<meta name="twitter:image" content="${SITE_URL}/og.png">
<link rel="icon" href="/icon.svg" type="image/svg+xml">
<link rel="icon" href="/favicon.png" type="image/png" sizes="64x64">
<link rel="apple-touch-icon" href="/apple-touch-icon.png">
<link rel="stylesheet" href="/vendor/oat/oat.min.css">
<link rel="stylesheet" href="/assets/site.css">
<script src="/vendor/oat/oat.min.js" defer></script>
<script src="/assets/site.js" defer></script>
<script>try{var t=localStorage.getItem("torrz-theme");if(t)document.documentElement.setAttribute("data-theme",t)}catch(e){}</script>
</head>`;
}

/** The top bar, Oat's [data-topnav]. The docs add the button that opens their sidebar on phones. */
function header(current, { sidebar = false } = {}) {
  const link = (href, text, key, cls = "") => `<a href="${href}"${cls ? ` class="${cls}"` : ""}${current === key ? ' aria-current="page"' : ""}>${text}</a>`;
  return `<header data-topnav>
  ${sidebar ? `<button data-sidebar-toggle class="ghost icon small" aria-label="Show the docs menu">${icons.menu}</button>` : ""}
  <a class="brand" href="/"><img src="/icon.svg" alt="" width="28" height="28">Torrz</a>
  <nav class="hstack topnav-links" aria-label="Main">
    ${link("/#features", "Features", "features", "hide-sm")}
    ${link("/docs/", "Docs", "docs")}
    ${link("/docs/cli/", "CLI", "cli", "hide-sm")}
    <a class="button ghost small" href="${GITHUB}" rel="noopener">${icons.github}<span class="hide-sm">GitHub</span></a>
    <button class="ghost icon small theme-toggle" type="button" aria-label="Switch between light and dark">${icons.moon}${icons.sun}</button>
  </nav>
</header>`;
}

function footer() {
  return `<footer class="site-footer">
  <div class="container">
    <div class="row">
      <div class="col-6">
        <a class="brand" href="/"><img src="/icon.svg" alt="" width="24" height="24">Torrz</a>
        <p>A BitTorrent client where every button is also a command. MIT licensed. This site is built with <a href="https://oat.ink" rel="noopener">Oat</a>.</p>
      </div>
      <nav class="col-6 hstack gap-4" aria-label="Footer">
        <a href="/docs/">Docs</a>
        <a href="/docs/cli/">CLI reference</a>
        <a href="${GITHUB}" rel="noopener">GitHub</a>
        <a href="${GITHUB}/issues" rel="noopener">Issues</a>
        <a href="${BLOB}/CHANGELOG.md" rel="noopener">Changelog</a>
        <a href="${BLOB}/LICENSE" rel="noopener">License</a>
      </nav>
    </div>
  </div>
</footer>`;
}

function fill(template, values) {
  return template.replace(/\{\{(\w+)\}\}/g, (_, k) => {
    if (!(k in values)) throw new Error(`no value for {{${k}}}`);
    return values[k];
  });
}

function docsPage(page, i) {
  const md = readFileSync(join(repo, "docs", page.file), "utf8");
  const { title, body, headings } = render(md);
  const groups = [...new Set([...PAGES, ...EXTERNAL].map((p) => p.group))];
  const navItem = (p) =>
    "slug" in p
      ? `<li><a href="${pageUrl(p.slug)}"${p === page ? ' aria-current="page"' : ""}>${esc(p.nav)}</a></li>`
      : `<li><a href="${p.href}" rel="noopener">${esc(p.nav)}<span class="text-lighter">↗</span></a></li>`;
  // Oat's sidebar nav: a group a <details>, open, as Oat's own docs do it.
  const sidebar = `<ul>${groups
    .map((g) => `<li><details open><summary>${g}</summary><ul>${[...PAGES, ...EXTERNAL].filter((p) => p.group === g).map(navItem).join("")}</ul></details></li>`)
    .join("")}</ul>`;
  // A command's heading lists its arguments; the contents list needs only the command.
  const tocLabel = (html) => {
    const text = html.replace(/<[^>]+>/g, "");
    return text.startsWith("torrz ") ? text.split(/ (?=\[|&lt;|or |and |--|\(alias)/)[0] : text;
  };
  const toc = headings.length
    ? `<h2>On this page</h2><ul>${headings.map((h) => `<li><a href="#${h.id}"${h.depth === 3 ? ' class="sub"' : ""}>${tocLabel(h.html)}</a></li>`).join("")}</ul>`
    : "";
  const prev = PAGES[i - 1];
  const next = PAGES[i + 1];
  const foot = `<nav class="page-foot" aria-label="Pages">
    <span>${prev ? `<a href="${pageUrl(prev.slug)}">← ${esc(prev.nav)}</a>` : ""}</span>
    <span>${next ? `<a href="${pageUrl(next.slug)}">${esc(next.nav)} →</a>` : `<a href="${BLOB}/docs/${page.file}" rel="noopener">Edit this page on GitHub</a>`}</span>
  </nav>`;
  const firstPara = (md.split(/\n\n/).find((b) => b && !b.startsWith("#")) || "").replace(/[`*\[\]]|\(.*?\)/g, "").replace(/\s+/g, " ").trim();
  const description = firstPara.length > 160 ? firstPara.slice(0, 157).replace(/\s+\S*$/, "") + "…" : firstPara;
  const path = pageUrl(page.slug);
  return `${head({ title: `${title} · Torrz docs`, description, path })}
<body data-sidebar-layout>
${header(page.slug === "cli" ? "cli" : "docs", { sidebar: true })}
<aside data-sidebar aria-label="Docs">
  <nav>${sidebar}</nav>
</aside>
<main>
  <div class="docs-body">
    <article class="prose">
      ${body}
      ${foot}
    </article>
    <aside class="toc" aria-label="On this page"><nav>${toc}</nav></aside>
  </div>
  ${footer()}
</main>
</body>
</html>
`;
}

function write(path, html) {
  const file = join(out, path);
  mkdirSync(dirname(file), { recursive: true });
  writeFileSync(file, html);
}

// The docs are rebuilt from scratch each time, so a page removed from PAGES disappears.
if (existsSync(join(out, "docs"))) rmSync(join(out, "docs"), { recursive: true });

const landing = readFileSync(join(here, "src", "index.html"), "utf8");
write(
  "index.html",
  fill(landing, {
    head: head({
      title: "Torrz: a BitTorrent client where every click is a command",
      description: "Torrz is an open-source BitTorrent client for macOS. Every button in the window has a matching command, so anything you can click, you can script.",
      path: "/",
    }),
    header: header("home"),
    footer: footer(),
    github: GITHUB,
  }),
);
write(
  "404.html",
  fill(readFileSync(join(here, "src", "404.html"), "utf8"), {
    head: head({ title: "Not found · Torrz", description: "There's no page at this address.", path: "/404" }),
    header: header(""),
    footer: footer(),
  }),
);
PAGES.forEach((p, i) => write(join(pageUrl(p.slug).slice(1), "index.html"), docsPage(p, i)));

const urls = ["/", ...PAGES.map((p) => pageUrl(p.slug))];
write("sitemap.xml", `<?xml version="1.0" encoding="UTF-8"?>\n<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">\n${urls.map((u) => `  <url><loc>${SITE_URL}${u}</loc></url>`).join("\n")}\n</urlset>\n`);
write("robots.txt", `User-agent: *\nAllow: /\n\nSitemap: ${SITE_URL}/sitemap.xml\n`);
console.log(`built ${urls.length} pages for ${SITE_URL}`);
