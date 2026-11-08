// Ordena y etiqueta las secciones de la barra lateral generada a partir de las carpetas
// de docs/user/<idioma>/. La carpeta de cada sección se deduce del identificador de sus
// páginas (p. ej. "concepts/a2a"); la etiqueta, del nombre de la carpeta y no del título
// de su índice.

type Elemento = {
  type: string;
  id?: string;
  label?: string;
  link?: {type: string; id?: string};
  items?: Elemento[];
  [clave: string]: unknown;
};

type Argumentos = {
  defaultSidebarItemsGenerator: (argumentos: unknown) => Promise<Elemento[]>;
  version?: {contentPath?: string};
  [clave: string]: unknown;
};

type Idioma = 'en' | 'es';

// Orden de las secciones, por nombre de carpeta.
const ORDEN_SECCIONES = ['start', 'concepts', 'guides', 'reference', 'problems'];

const ETIQUETAS: Record<Idioma, Record<string, string>> = {
  en: {
    start: 'Start',
    concepts: 'Concepts',
    guides: 'Guides',
    reference: 'Reference',
    problems: 'Problems',
  },
  es: {
    start: 'Empezar',
    concepts: 'Conceptos',
    guides: 'Guías',
    reference: 'Referencia',
    problems: 'Problemas',
  },
};

// Identificador de la primera página de una sección (la de su índice, si la tiene).
function primerIdDoc(elemento: Elemento): string | undefined {
  if (elemento.type === 'doc' && typeof elemento.id === 'string') {
    return elemento.id;
  }
  if (elemento.link?.type === 'doc' && typeof elemento.link.id === 'string') {
    return elemento.link.id;
  }
  for (const hijo of elemento.items ?? []) {
    const id = primerIdDoc(hijo);
    if (id) {
      return id;
    }
  }
  return undefined;
}

// La instancia en español lee docs/user/es; la de inglés, docs/user/en.
function idiomaDe(argumentos: Argumentos): Idioma {
  const ruta = argumentos.version?.contentPath ?? '';
  return /[\\/]es$/.test(ruta) ? 'es' : 'en';
}

function carpetaDe(elemento: Elemento): string | undefined {
  if (elemento.type !== 'category') {
    return undefined;
  }
  const id = primerIdDoc(elemento);
  return id ? id.split('/')[0] : undefined;
}

function rango(carpeta: string | undefined): number {
  const indice = carpeta ? ORDEN_SECCIONES.indexOf(carpeta) : -1;
  return indice === -1 ? ORDEN_SECCIONES.length : indice;
}

function etiquetar(elemento: Elemento, idioma: Idioma): Elemento {
  const carpeta = carpetaDe(elemento);
  const etiqueta = carpeta ? ETIQUETAS[idioma][carpeta] : undefined;
  return etiqueta ? {...elemento, label: etiqueta} : elemento;
}

export default async function ordenarBarra(argumentos: Argumentos): Promise<Elemento[]> {
  const elementos = await argumentos.defaultSidebarItemsGenerator(argumentos);
  const idioma = idiomaDe(argumentos);
  return elementos
    .map((elemento, posicion) => ({elemento, posicion, carpeta: carpetaDe(elemento)}))
    .sort((a, b) => rango(a.carpeta) - rango(b.carpeta) || a.posicion - b.posicion)
    .map(({elemento}) => etiquetar(elemento, idioma));
}
