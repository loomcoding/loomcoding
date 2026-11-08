// Plugin remark: reescribe los enlaces relativos de docs/user/<idioma>/ que salen de esa
// carpeta (documentos técnicos del repositorio) hacia su URL en GitHub. Las imágenes
// (p. ej. ../../img/*.svg) se dejan a Docusaurus, que las incorpora al build.
import path from 'node:path';

type Nodo = {
  type: string;
  url?: string;
  children?: Nodo[];
};

export type Opciones = {
  raizDocs: string;
  raizRepo: string;
  urlRepo: string;
};

const IMAGEN = /\.(svg|png|jpe?g|gif|webp)$/i;
// Esquemas (https:, mailto:), rutas absolutas y enlaces de ancla no se tocan.
const NO_RELATIVO = /^([a-z][a-z0-9+.-]*:|\/|#)/i;

function visitar(nodo: Nodo, visitante: (nodo: Nodo) => void): void {
  visitante(nodo);
  for (const hijo of nodo.children ?? []) {
    visitar(hijo, visitante);
  }
}

export default function remarkEnlacesFueraDeDocs(opciones: Opciones) {
  return (arbol: Nodo, archivo: {path?: string}): void => {
    if (!archivo.path) {
      return;
    }
    const carpeta = path.dirname(archivo.path);

    visitar(arbol, (nodo) => {
      if (nodo.type !== 'link' || !nodo.url || NO_RELATIVO.test(nodo.url)) {
        return;
      }
      const indiceAncla = nodo.url.indexOf('#');
      const ruta = indiceAncla === -1 ? nodo.url : nodo.url.slice(0, indiceAncla);
      const ancla = indiceAncla === -1 ? '' : nodo.url.slice(indiceAncla);
      if (ruta === '' || IMAGEN.test(ruta)) {
        return;
      }

      const destino = path.resolve(carpeta, ruta);
      if (destino === opciones.raizDocs || destino.startsWith(opciones.raizDocs + path.sep)) {
        return;
      }

      const relativo = path.relative(opciones.raizRepo, destino);
      if (relativo.startsWith('..') || path.isAbsolute(relativo)) {
        throw new Error(`Enlace fuera del repositorio: "${nodo.url}" en ${archivo.path}`);
      }
      nodo.url = `${opciones.urlRepo}/blob/master/${relativo.split(path.sep).join('/')}${ancla}`;
    });
  };
}
