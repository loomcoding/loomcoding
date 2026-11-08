import Head from '@docusaurus/Head';
import {Redirect} from '@docusaurus/router';

// La portada en español es la primera página de la guía en español. La redirección va en
// el HTML (meta refresh) para que funcione también sin JavaScript.
export default function Inicio() {
  return (
    <>
      <Head>
        <meta http-equiv="refresh" content="0; url=/es/start" />
        <link rel="canonical" href="https://loomcoding.com/es/start" />
      </Head>
      <Redirect to="/es/start" />
    </>
  );
}
