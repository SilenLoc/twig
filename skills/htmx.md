</> htmx - high power tools for html

[</**\> htm**x**](/)

[docs](/docs/)

[reference](/reference/)

[examples](/examples/)

[talk](/talk/)

[essays](/essays/)

 Search 

[star](https://github.com/bigskysoftware/htmx)

 
![Ad: MacMall PowerBooks as low as 1999$! Call 888-932-1569. Get your FREE 64MB RAM with your PowerBook!](/img/ads_top.png)

</\> htmx *high power tools for HTML*

![shut up warren ⁺₊✦ uwu](/img/wuw.png)

![htmx エイチティーエムエックス uwu](/img/kawaii.png)

![Ads: Get Flash! FREE Microsoft Internet Explorer! Netscape Now! (3.0) Site created with Microsoft® FrontPage™. Powered by Microsoft BackOffice.](/img/ads_bottom.png)

## introduction

htmx gives you access to [AJAX](https://htmx.org/docs/#ajax), [CSS Transitions](https://htmx.org/docs/#css_transitions), [WebSockets](https://htmx.org/docs/#websockets-and-sse) and [Server Sent Events](https://htmx.org/docs/#websockets-and-sse) directly in HTML, using [attributes](https://htmx.org/reference/#attributes), so you can build [modern user interfaces](https://htmx.org/examples/) with the [simplicity](https://en.wikipedia.org/wiki/HATEOAS) and [power](https://www.ics.uci.edu/~fielding/pubs/dissertation/rest_arch_style.htm) of hypertext

htmx is small ([~16k min.gz’d](https://cdn.jsdelivr.net/npm/htmx.org/dist/)), [dependency-free](https://github.com/bigskysoftware/htmx/blob/master/package.json), [extendable](https://htmx.org/extensions) & has **reduced** code base sizes by [67% when compared with react](https://htmx.org/essays/a-real-world-react-to-htmx-port/)

## motivation

-   Why should only [`<a>`](https://developer.mozilla.org/en-US/docs/Web/HTML/Element/a) & [`<form>`](https://developer.mozilla.org/en-US/docs/Web/HTML/Element/form) be able to make HTTP requests?
-   Why should only [`click`](https://developer.mozilla.org/en-US/docs/Web/API/Element/click_event) & [`submit`](https://developer.mozilla.org/Web/API/HTMLFormElement/submit_event) events trigger them?
-   Why should only [`GET`](https://developer.mozilla.org/en-US/docs/Web/HTTP/Methods/GET) & [`POST`](https://developer.mozilla.org/en-US/docs/Web/HTTP/Methods/POST) methods be [available](https://developer.mozilla.org/en-US/docs/Web/HTTP/Methods)?
-   Why should you only be able to replace the **entire** screen?

By removing these constraints, htmx completes HTML as a [hypertext](https://en.wikipedia.org/wiki/Hypertext)

## quick start

```html
  <script src="https://cdn.jsdelivr.net/npm/htmx.org@2.0.8/dist/htmx.min.js"></script>
  <!-- have a button POST a click via AJAX -->
  <button hx-post="/clicked" hx-swap="outerHTML">
    Click Me
  </button>
```

The [`hx-post`](https://htmx.org/attributes/hx-post/) and [`hx-swap`](https://htmx.org/attributes/hx-swap/) attributes on this button tell htmx:

> “When a user clicks on this button, issue an AJAX request to /clicked, and replace the entire button with the HTML response”

htmx is the successor to [intercooler.js](http://intercoolerjs.org)

Read the [docs introduction](https://htmx.org/docs/#introduction) for a more in-depth… introduction.

Note that htmx 2.x has dropped IE support. If you require IE support you can use the [1.x](https://v1.htmx.org) code-line, which will be supported in perpetuity.

## book

We are happy to announce the release of [Hypermedia Systems](https://hypermedia.systems), a book on how to build [Hypermedia-Driven Applications](https://htmx.org/essays/hypermedia-driven-applications/) using htmx & more:

![Hypermedia Systems hardcover edition](/img/hypermedia-systems.png)

## sponsors

htmx development can be supported via [GitHub Sponsors](https://github.com/sponsors/bigskysoftware?o=esb)

Thank you to all our generous [supporters](https://github.com/sponsors/bigskysoftware?o=esb), including:

# Platinum Sponsor

 ![commspace](/img/commspace.svg) ![commspace](/img/commspace-dark.svg)

## [Silver Sponsors](#silver-sponsors)

![Jetbrains](/img/jetbrains.svg)

![GitHub](/img/Github_Logo.png)

 ![craft cms](/img/logo-craft-cms.svg) ![craft cms](/img/logo-craft-cms-dark.svg)

![ButterCMS](/img/butter-cms.svg)

![Black Host](/img/blackhost-logo.svg)

 ![V7N](/img/v7n-logo.png) ![V7N](/img/v7n-logo-dark.png)

![Hiro The Doggo](/img/sekun-doggo.jpg)

 ![Das Filter](/img/das-filter.svg) ![Das Filter](/img/das-filter-dark.svg)

 ![PullApprove](/img/pullapprove-logo.svg) ![PullApprove](/img/pullapprove-logo-dark.svg)

 ![UI Bakery](/img/ui-bakery.svg) ![UI Bakery](/img/ui-bakery-dark.svg)

 ![Tracebit Cloud Canaries](/img/tracebit-logo.png) ![Tracebit Cloud Canaries](/img/tracebit-logo-dark.png)

![RxDB JavaScript Database](/img/rxdb.svg)

![Ohne-Makler](/img/ohne-makler.svg)

 ![Developer friendly DevOps](/img/Cased_Logo_DarkBlue.svg) ![Developer friendly DevOps](/img/Cased_Logo_Beige-01.svg)

![How to start an LLC - a guide from LLC.org, proud open source sponsor](/img/llc-org.svg)

 ![VPS Server Hosting in the Cloud: Cost Efficiency](/img/vpsserver-logo.svg) ![VPS Server Hosting in the Cloud: Cost Efficiency](/img/vps-server-logo-dark.svg)

![](/img/ablogcms_logo.svg)

 ![](/img/BroadbandMapLogo2LineLightMode.png) ![](/img/BroadbandMapLogo2LineDarkMode.png)

 ![](/img/follower_light.svg) ![](/img/follower_dark.svg)

 ![](/img/exchange-rate-api.png) ![](/img/exchange-rate-api-dark.png)

 ![](/img/rsz_instant_famous.png)

 ![](/img/stake.jpeg)

ʕ •ᴥ•ʔ made in montana

## haiku

*javascript fatigue:  
longing for a hypertext  

[docs](/docs/)

[reference](/reference/)

[examples](/examples/)

[talk](/talk/)

[essays](/essays/)

[@htmx\_org](https://twitter.com/htmx_org)

![](/img/bss_bars.png)
