# TWZRD Documentation Site

Static documentation site for TWZRD Attention Oracle.

## Structure

```
docs-site/
├── index.html           # Home page
├── style.css            # Global styles
├── getting-started/     # Getting started guide
├── api/                 # API reference
├── cli/                 # CLI documentation
└── guides/              # Integration guides
```

## Deployment Options

### Option 1: Netlify (Recommended)

1. Create Netlify account
2. Connect GitHub repo
3. Configure build settings:
   ```
   Base directory: docs-site
   Build command: (leave empty)
   Publish directory: .
   ```
4. Set custom domain: `docs.twzrd.xyz`
5. Add DNS record:
   ```
   Type: CNAME
   Name: docs
   Value: [your-site].netlify.app
   ```

### Option 2: Vercel

1. Install Vercel CLI: `npm i -g vercel`
2. Navigate to docs-site: `cd docs-site`
3. Deploy: `vercel --prod`
4. Set custom domain in dashboard
5. Add DNS record:
   ```
   Type: CNAME
   Name: docs
   Value: cname.vercel-dns.com
   ```

### Option 3: GitHub Pages

1. Create `gh-pages` branch
2. Copy docs-site contents to root
3. Enable Pages in repo settings
4. Source: gh-pages branch
5. Custom domain: docs.twzrd.xyz
6. Add CNAME file with domain
7. Add DNS record:
   ```
   Type: CNAME
   Name: docs
   Value: [username].github.io
   ```

### Option 4: Self-Hosted (Nginx)

Add to your Nginx config:

```nginx
server {
    listen 443 ssl http2;
    server_name docs.twzrd.xyz;

    ssl_certificate /etc/letsencrypt/live/docs.twzrd.xyz/fullchain.pem;
    ssl_certificate_key /etc/letsencrypt/live/docs.twzrd.xyz/privkey.pem;

    root /var/www/docs-site;
    index index.html;

    location / {
        try_files $uri $uri/ /index.html;
    }

    # Security headers
    add_header X-Frame-Options "SAMEORIGIN" always;
    add_header X-Content-Type-Options "nosniff" always;
    add_header X-XSS-Protection "1; mode=block" always;
    add_header Strict-Transport-Security "max-age=31536000; includeSubDomains; preload" always;
}
```

Then:
```bash
sudo cp -r docs-site /var/www/
sudo systemctl reload nginx
```

## DNS Configuration

Add this DNS record once deployed:

```
Type: CNAME
Name: docs
Value: [your-deployment-url]
TTL: 3600
```

## Testing Locally

Since this is a static site, you can test with any local server:

```bash
# Python
cd docs-site
python3 -m http.server 8000

# Node.js
npx serve docs-site

# PHP
php -S localhost:8000 -t docs-site
```

Then visit: http://localhost:8000

## Updating Content

1. Edit HTML files directly
2. Commit changes
3. Push to repository
4. Auto-deploys (if configured with Netlify/Vercel)

## SEO Optimization

Already included:
- ✅ Semantic HTML5
- ✅ Meta descriptions
- ✅ Fast load times (<10KB total)
- ✅ Mobile responsive
- ✅ Clean URLs

To improve:
1. Add sitemap.xml for docs pages
2. Submit to Google Search Console
3. Add schema.org markup
4. Generate OG images for social shares

## Analytics (Optional)

Add to each page's `<head>` if desired:

```html
<!-- Plausible (privacy-friendly) -->
<script defer data-domain="docs.twzrd.xyz" src="https://plausible.io/js/script.js"></script>

<!-- OR Google Analytics -->
<script async src="https://www.googletagmanager.com/gtag/js?id=G-XXXXXXXXXX"></script>
```

## License

MIT © TWZRD Inc.

---

**Status:** Ready to deploy
**Last Updated:** 2025-11-11
**Contact:** dev@twzrd.xyz
