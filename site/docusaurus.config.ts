import path from 'node:path';
import {themes as prismThemes} from 'prism-react-renderer';
import type {Config} from '@docusaurus/types';
import type * as Preset from '@docusaurus/preset-classic';
import remarkEnlacesFueraDeDocs from './plugins/remark-enlaces-fuera-de-docs';
import ordenarBarra from './sidebar-orden';

// Este fichero se ejecuta en Node.js: nada de código de navegador aquí.

// Repositorio de la organización. Mientras sea reservado, estos enlaces solo los abre quien tenga acceso.
const REPO_URL = 'https://github.com/loomcoding/loomcoding';

// El sitio vive en site/ y el contenido en docs/user/ del repositorio, sin copias.
const RAIZ_REPO = path.resolve(__dirname, '..');
const DOCS_EN = path.join(RAIZ_REPO, 'docs', 'user', 'en');
const DOCS_ES = path.join(RAIZ_REPO, 'docs', 'user', 'es');

// Ficheros que empiezan por "_" (plantillas) no son páginas.
const EXCLUIR = ['**/_*.md', '**/_*/**'];

const instanciaDocs = (id: string, carpeta: string, routeBasePath: string) => [
  '@docusaurus/plugin-content-docs',
  {
    id,
    path: carpeta,
    routeBasePath,
    sidebarPath: './sidebars.ts',
    sidebarItemsGenerator: ordenarBarra,
    exclude: EXCLUIR,
    // Antes de los plugins remark de Docusaurus, que son los que validan los enlaces.
    beforeDefaultRemarkPlugins: [
      [
        remarkEnlacesFueraDeDocs,
        {raizDocs: carpeta, raizRepo: RAIZ_REPO, urlRepo: REPO_URL},
      ],
    ],
  },
];

const config: Config = {
  title: 'Loomcoding (experimental)',
  tagline: 'Experimental · User documentation / Documentación de usuario',

  url: 'https://loomcoding.com',
  baseUrl: '/',
  trailingSlash: false,

  // Un enlace roto debe impedir la construcción.
  onBrokenLinks: 'throw',

  markdown: {
    format: 'detect',
    mermaid: true,
    hooks: {
      onBrokenMarkdownLinks: 'throw',
    },
  },
  themes: [
    [
      '@easyops-cn/docusaurus-search-local',
      {
        // Indexa las dos instancias de documentación (inglés en /, español en /es).
        docsDir: [DOCS_EN, DOCS_ES],
        docsRouteBasePath: ['/', 'es'],
        language: ['en', 'es'],
        indexDocs: true,
        indexBlog: false,
        indexPages: false,
        highlightSearchTermsOnTargetPage: true,
        explicitSearchResultPath: true,
      },
    ],
    '@docusaurus/theme-mermaid',
  ],
  // Dos instancias del plugin de docs: inglés en la raíz y español en /es.
  plugins: [
    instanciaDocs('user-en', DOCS_EN, '/'),
    instanciaDocs('user-es', DOCS_ES, 'es'),
  ],

  presets: [
    [
      'classic',
      {
        docs: false,
        blog: false,
        theme: {
          customCss: './src/css/custom.css',
        },
      } satisfies Preset.Options,
    ],
  ],

  themeConfig: {
    colorMode: {
      respectPrefersColorScheme: true,
    },
    navbar: {
      title: 'Loomcoding · experimental',
      items: [
        {
          to: '/',
          label: 'English',
          position: 'right',
          activeBaseRegex: '^/(?!es(/|$))',
        },
        {
          // Sin barra final: con trailingSlash en false el build genera es.html, no es/index.html.
          to: '/es',
          label: 'Español',
          position: 'right',
          activeBaseRegex: '^/es(/|$)',
        },
        {
          href: REPO_URL,
          label: 'GitHub',
          position: 'right',
        },
      ],
    },
    footer: {
      style: 'light',
      copyright: `Loomcoding, documentación de usuario. © ${new Date().getFullYear()}`,
    },
    prism: {
      theme: prismThemes.github,
      darkTheme: prismThemes.dracula,
    },
  } satisfies Preset.ThemeConfig,
};

export default config;
