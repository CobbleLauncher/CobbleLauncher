# CobbleLauncher publisher website

Static Cloudflare Pages site at https://cobble.yuzhes.com.

- `/`: temporary under-construction landing page
- `/terms`: terms of service
- `/privacy`: privacy statement for the current website and launcher features
- `/.well-known/microsoft-identity-association.json`: Microsoft publisher-domain verification

## Build

```sh
python3 website/build.py
```

From this standalone directory, use `python3 build.py` instead. The build preserves the verification JSON in `public/.well-known/`.

## Deploy

The current deployment uses direct uploads, not a Git integration. With an authorized Cloudflare token and account configured in your environment:

```sh
wrangler pages deploy website/public --project-name cobble-publisher --branch main
```

Keep credentials outside this repository. The custom domain is `cobble.yuzhes.com`, pointing to `cobble-publisher.pages.dev`.

Verify the live homepage, both legal routes, and the JSON content after every deployment. The verification endpoint must return the JSON directly over HTTPS without login or redirects.

The legal pages currently describe early-development software without Microsoft sign-in. Update the privacy statement before adding account sign-in, token storage, telemetry, cloud sync, or other new data processing. Contact email: `a@yuzhes.com`.
