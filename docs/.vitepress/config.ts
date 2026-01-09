import { defineConfig } from 'vitepress'
import { withMermaid } from 'vitepress-plugin-mermaid'

export default withMermaid(
  defineConfig({
    title: 'rescribe',
    description: 'Universal document conversion library',

    base: '/rescribe/',

    themeConfig: {
      nav: [
        { text: 'Guide', link: '/introduction' },
        { text: 'API', link: '/api' },
        { text: 'Formats', link: '/formats' },
      ],

      sidebar: [
        {
          text: 'Guide',
          items: [
            { text: 'Introduction', link: '/introduction' },
            { text: 'Quickstart', link: '/quickstart' },
          ]
        },
        {
          text: 'Concepts',
          items: [
            { text: 'Document Model', link: '/document-model' },
            { text: 'Properties', link: '/properties' },
            { text: 'Resources', link: '/resources' },
            { text: 'Fidelity Tracking', link: '/fidelity' },
          ]
        },
        {
          text: 'Reference',
          items: [
            { text: 'API', link: '/api' },
            { text: 'Formats', link: '/formats' },
            { text: 'Transformers', link: '/transformers' },
          ]
        },
      ],

      socialLinks: [
        { icon: 'github', link: 'https://github.com/pterror/rescribe' }
      ],

      search: {
        provider: 'local'
      },

      editLink: {
        pattern: 'https://github.com/pterror/rescribe/edit/master/docs/:path',
        text: 'Edit this page on GitHub'
      },
    },

    vite: {
      optimizeDeps: {
        include: ['mermaid'],
      },
    },
  }),
)
