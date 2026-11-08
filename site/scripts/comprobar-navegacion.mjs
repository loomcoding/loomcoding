// Comprueba que cada página de docs/user/{en,es}/ tiene su HTML en build/ y que
// aparece enlazada desde la barra lateral de su idioma. Sin dependencias.
// Uso: npm run build && npm run check

import {existsSync, readFileSync, readdirSync, statSync} from 'node:fs';
import {dirname, join, relative, resolve, sep} from 'node:path';
import {fileURLToPath} from 'node:url';

const SITIO = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const BUILD = join(SITIO, 'build');
const DOCS = join(SITIO, '..', 'docs', 'user');

const IDIOMAS = [
  {codigo: 'en', carpeta: join(DOCS, 'en'), prefijo: ''},
  {codigo: 'es', carpeta: join(DOCS, 'es'), prefijo: '/es'},
];

function paginasFuente(carpeta) {
  const resultado = [];
  const recorrer = (directorio) => {
    for (const nombre of readdirSync(directorio)) {
      const ruta = join(directorio, nombre);
      if (nombre.startsWith('_')) {
        continue;
      }
      if (statSync(ruta).isDirectory()) {
        recorrer(ruta);
      } else if (nombre.endsWith('.md')) {
        resultado.push(ruta);
      }
    }
  };
  recorrer(carpeta);
  return resultado.sort();
}

// Ruta de la página publicada (sin barra final, como genera el sitio).
function rutaPublicada(rutaFuente, idioma) {
  const relativa = relative(idioma.carpeta, rutaFuente).split(sep).join('/');
  let sinExtension = relativa.replace(/\.md$/, '');
  if (sinExtension === 'index') {
    sinExtension = '';
  } else if (sinExtension.endsWith('/index')) {
    sinExtension = sinExtension.slice(0, -'/index'.length);
  }
  return sinExtension === '' ? idioma.prefijo || '/' : `${idioma.prefijo}/${sinExtension}`;
}

function ficheroHtml(ruta) {
  const base = ruta === '/' ? '' : ruta.slice(1);
  const candidatos = [join(BUILD, `${base}.html`), join(BUILD, base, 'index.html')];
  return candidatos.find((candidato) => existsSync(candidato)) ?? null;
}

// Enlaces de la barra lateral (<aside class="theme-doc-sidebar-container">) de todas las
// páginas del idioma. Docusaurus pinta la misma barra en cada página de su instancia.
function enlacesBarraLateral(htmlFiles) {
  const enlaces = new Set();
  for (const fichero of htmlFiles) {
    const html = readFileSync(fichero, 'utf8');
    const barra = html.match(/<aside class="theme-doc-sidebar-container[^"]*"[\s\S]*?<\/aside>/);
    if (!barra) {
      continue;
    }
    for (const etiqueta of barra[0].match(/<a [^>]*menu__link[^>]*>/g) ?? []) {
      const href = etiqueta.match(/href="([^"]*)"/);
      if (href && href[1].startsWith('/')) {
        enlaces.add(href[1].split('#')[0].split('?')[0].replace(/\/$/, '') || '/');
      }
    }
  }
  return enlaces;
}

function htmlDelIdioma(idioma) {
  const salida = [];
  const recorrer = (directorio) => {
    for (const nombre of readdirSync(directorio)) {
      const ruta = join(directorio, nombre);
      if (statSync(ruta).isDirectory()) {
        recorrer(ruta);
      } else if (nombre.endsWith('.html')) {
        salida.push(ruta);
      }
    }
  };
  if (idioma.prefijo) {
    const carpetaIdioma = join(BUILD, idioma.prefijo.slice(1));
    if (existsSync(carpetaIdioma)) {
      recorrer(carpetaIdioma);
    }
  } else {
    for (const nombre of readdirSync(BUILD)) {
      const ruta = join(BUILD, nombre);
      if (statSync(ruta).isDirectory()) {
        if (nombre !== 'es' && nombre !== 'assets') {
          recorrer(ruta);
        }
      } else if (nombre.endsWith('.html')) {
        salida.push(ruta);
      }
    }
  }
  return salida;
}

function main() {
  if (!existsSync(BUILD)) {
    console.error('No existe build/. Ejecuta antes «npm run build».');
    process.exit(2);
  }

  const errores = [];
  let total = 0;

  for (const idioma of IDIOMAS) {
    const enBarra = enlacesBarraLateral(htmlDelIdioma(idioma));
    if (enBarra.size === 0) {
      errores.push(`[${idioma.codigo}] la barra lateral no tiene ningún enlace en build/`);
    }

    for (const fuente of paginasFuente(idioma.carpeta)) {
      total += 1;
      const ruta = rutaPublicada(fuente, idioma);
      const etiqueta = `[${idioma.codigo}] ${relative(DOCS, fuente).split(sep).join('/')}`;
      if (!ficheroHtml(ruta)) {
        errores.push(`${etiqueta}: falta el HTML de ${ruta} en build/`);
        continue;
      }
      if (!enBarra.has(ruta)) {
        errores.push(`${etiqueta}: ${ruta} no aparece en la barra lateral`);
      }
    }
  }

  if (errores.length > 0) {
    console.error(`Navegación incompleta (${errores.length}):`);
    for (const error of errores) {
      console.error(`  - ${error}`);
    }
    process.exit(1);
  }
  console.log(`Navegación completa: ${total} páginas con HTML y enlazadas desde la barra lateral.`);
}

main();
