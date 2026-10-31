declare module "astro-solid-next" {
  import type { AstroIntegration } from "astro";
  export default function solidNext(options?: {
    include?: string | string[];
    exclude?: string | string[];
  }): AstroIntegration;
}
