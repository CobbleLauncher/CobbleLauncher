from pathlib import Path
from html import escape

ROOT = Path(__file__).parent / 'public'
REPO = 'https://github.com/CobbleLauncher/CobbleLauncher'
CSS = '''
:root{color-scheme:light;--bg:#f7f8f4;--ink:#242923;--muted:#626c5c;--line:#dde3d6;--accent:#c2e781;--button-ink:#253b18}
*{box-sizing:border-box}body{margin:0;background:var(--bg);color:var(--ink);font:16px/1.85 -apple-system,BlinkMacSystemFont,'Segoe UI','Microsoft YaHei',sans-serif;-webkit-font-smoothing:antialiased}a{color:inherit;text-underline-offset:5px}a:focus-visible{outline:2px solid #688d3c;outline-offset:5px}header,footer{max-width:1120px;margin:auto;padding:25px 32px}header{display:flex;justify-content:space-between;align-items:center;border-bottom:1px solid var(--line);gap:18px}.brand{font-size:18px;letter-spacing:-.6px;font-weight:750;text-decoration:none;display:flex;align-items:center;gap:11px}.mark{display:grid;grid-template-columns:repeat(2,7px);gap:3px;transform:rotate(-8deg)}.mark i{width:7px;height:7px;background:#8caf60;border-radius:1px}.mark i:nth-child(2){opacity:.65}.mark i:nth-child(4){opacity:.4}.repo{font-size:13px;text-decoration:none;min-height:44px;display:inline-flex;align-items:center}.hero{max-width:1120px;margin:auto;padding:100px 32px 110px;min-height:65vh;display:flex;flex-direction:column;justify-content:center}.eyebrow{text-transform:uppercase;font:12px/1.6 ui-monospace,monospace;letter-spacing:2px;color:var(--muted);margin:0 0 24px}.status{display:inline-flex;align-items:center;gap:8px;font-size:13px;color:var(--muted);margin:0 0 22px}.status:before{content:'';width:7px;height:7px;background:#7a9a52;border-radius:50%}h1{font-size:clamp(42px,7vw,82px);line-height:1.12;font-weight:650;letter-spacing:-3px;margin:0 0 26px}.lead{font-size:18px;max-width:540px;color:var(--muted);margin:0 0 32px}.actions{display:flex;gap:12px;flex-wrap:wrap}.button{display:inline-flex;align-items:center;justify-content:center;min-height:46px;padding:9px 20px;border-radius:9px;font-size:14px;font-weight:600;text-decoration:none;background:var(--accent);color:var(--button-ink)}.button:hover{filter:brightness(.96)}.note{font-size:13px;color:var(--muted);margin:20px 0 0}footer{border-top:1px solid var(--line);font-size:12px;color:var(--muted)}.footer-row{display:flex;justify-content:space-between;gap:16px;align-items:center;flex-wrap:wrap}nav{display:flex;gap:22px;flex-wrap:wrap}nav a{min-height:44px;display:inline-flex;align-items:center}footer p{max-width:740px;line-height:1.7;margin:12px 0 0}.document{max-width:800px;margin:auto;padding:64px 32px 88px}.document h1{font-size:40px;letter-spacing:-1.5px;line-height:1.3;margin-bottom:12px}.meta{font-size:13px;color:var(--muted);margin-bottom:36px}.document h2{font-size:20px;line-height:1.5;margin:36px 0 12px;font-weight:650}.document p,.document ul{margin:12px 0}.document li{padding-left:3px;margin:7px 0}.intro{font-size:17px}.document a{overflow-wrap:anywhere}.back{font-size:13px;display:inline-block;margin-top:32px;min-height:44px} @media(max-width:600px){header,footer{padding:18px 22px}.brand{font-size:16px}.hero{padding:72px 24px 82px;min-height:65vh}h1{letter-spacing:-2px}.lead{font-size:16px}.document{padding:44px 24px 64px}.document h1{font-size:34px}.footer-row{gap:0}}
@media(prefers-color-scheme:dark){:root{color-scheme:dark;--bg:#171a17;--ink:#edf0e6;--muted:#a0aa98;--line:#30382e}}
'''


def page(title, description, body, path):
    canonical = 'https://cobble.yuzhes.com' + path
    return f'''<!doctype html>
<html lang="zh-CN"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>{escape(title)} · CobbleLauncher</title><meta name="description" content="{escape(description)}"><meta name="theme-color" content="#f7f8f4"><link rel="canonical" href="{canonical}"><style>{CSS}</style></head>
<body><header><a class="brand" href="/" aria-label="CobbleLauncher 首页"><span class="mark" aria-hidden="true"><i></i><i></i><i></i><i></i></span>CobbleLauncher</a><a class="repo" href="{REPO}">GitHub ↗</a></header>
{body}
<footer><div class="footer-row"><span>© 2026 CobbleLauncher contributors</span><nav aria-label="页脚"><a href="/terms">服务条款</a><a href="/privacy">隐私声明</a><a href="mailto:a@yuzhes.com">联系</a></nav></div><p>CobbleLauncher 是独立的开源社区项目，与 Mojang 或 Microsoft 无隶属、合作或背书关系。Minecraft 及相关商标归其各自权利人所有。</p></footer></body></html>'''

home = '''<main class="hero"><p class="eyebrow">Minecraft: Java Edition · Open source</p><p class="status">网站建设中</p><h1>先放下一块基石。</h1><p class="lead">CobbleLauncher，一款让你专注于游戏的 Minecraft 启动器。<br>我们正在搭建它的官方网站。</p><div class="actions"><a class="button" href="https://github.com/CobbleLauncher/CobbleLauncher">在 GitHub 关注项目 ↗</a></div><p class="note">项目仍处于早期开发阶段，暂未发布稳定版本。</p></main>'''
terms = '''<p class="intro">本条款适用于 CobbleLauncher 官方网站（cobble.yuzhes.com）及项目维护者提供的相关服务。启动器源代码的使用、复制、修改和分发由仓库中的 MIT 许可证规定。</p>
<h2>1. 项目与当前状态</h2><p>CobbleLauncher 是面向 Minecraft: Java Edition 的非官方开源启动器，由项目维护者及贡献者开发。目前项目处于早期开发阶段，尚无稳定版本。功能、兼容性和可用性可能发生变化，开发版本可能包含缺陷。</p><p>本网站目前提供项目介绍、文档链接与发布者域验证文件，不提供游戏账号注册、游戏销售或付费订阅服务。Microsoft 账号登录尚未上线。</p>
<h2>2. 使用条件</h2><p>你应遵守适用法律，以及 Minecraft、Microsoft 和你使用的第三方服务的相关条款。使用启动器不授予 Minecraft 的所有权或许可证，也不免除你取得相应授权的义务。离线玩家档案仅用于本地身份，不是已验证的 Microsoft 账号，不能用来访问要求正版账号认证的服务。</p><p>请勿利用本网站或相关服务实施未授权访问、传播恶意软件、侵犯他人权利或破坏服务。未成年人应在监护人指导下使用，并遵守所在地适用的年龄及同意要求。</p>
<h2>3. 第三方内容与服务</h2><p>启动器可能根据你的操作连接 Minecraft 官方下载服务、BMCLAPI、Modrinth、模组加载器服务或你自行配置的下载源。第三方服务、模组、整合包、资源包和光影包有各自的许可证、条款和隐私政策。项目维护者不控制这些服务，也不保证其持续可用或内容安全。</p><p>安装第三方内容前，请检查来源、权限、许可证及兼容性。不要在公开问题报告中上传密码、令牌或其他敏感信息。</p>
<h2>4. 本地文件与备份</h2><p>启动器可按你的操作创建、下载、更改或删除本地游戏文件和配置。更新、导入或删除游戏前，请备份存档及重要文件。不要将开发版本用于唯一一份重要数据。</p>
<h2>5. 开源许可与商标</h2><p>CobbleLauncher 的源代码以 <a href="https://github.com/CobbleLauncher/CobbleLauncher/blob/main/LICENSE">MIT 许可证</a>提供。第三方依赖和内容的许可证仍然适用。本条款不缩减 MIT 许可证授予你的权利。Minecraft、Mojang、Microsoft 及其他第三方名称和商标归各自权利人所有。</p>
<h2>6. 免责声明与责任</h2><p>在适用法律允许的范围内，软件和网站按“现状”和“可用状态”提供，不保证无错误、不中断、满足特定用途或与所有游戏版本及第三方内容兼容。软件相关的保证与责任限制以 MIT 许可证为准。本条款不排除或限制适用法律规定不得排除或限制的责任，也不影响你依法享有的强制性权利。</p>
<h2>7. 更新与联系</h2><p>网站和功能可能调整。本条款的重大变更将在本页更新并注明日期，变更不追溯削减已取得的开源许可权利。如有问题，请联系 <a href="mailto:a@yuzhes.com">a@yuzhes.com</a>，或在 <a href="https://github.com/CobbleLauncher/CobbleLauncher/issues">GitHub Issues</a> 提交非敏感问题。</p>'''
privacy = '''<p class="intro">本声明说明 CobbleLauncher 当前官方网站及现有启动器功能涉及的数据处理。网站由 CobbleLauncher 项目维护者提供，隐私问题可发送至 <a href="mailto:a@yuzhes.com">a@yuzhes.com</a>。</p>
<h2>1. 官方网站</h2><p>本网站是托管于 Cloudflare Pages 的静态网站，没有注册表单、评论功能或自建账号数据库。当前页面不加载广告、访问分析脚本、第三方字体或嵌入式社交组件，也不主动设置 Cookie 或使用浏览器本地存储。</p><p>为交付页面和保护网络，Cloudflare 会处理 IP 地址、请求 URL、时间、浏览器及连接信息，并可能根据其安全设置使用必要的安全机制。相关处理、存储地点与保留安排适用 <a href="https://www.cloudflare.com/privacypolicy/">Cloudflare 隐私政策</a>。本网站没有另行建立访客档案。</p>
<h2>2. 启动器中的本地数据</h2><p>现有启动器在设备上保存玩家档案、偏好设置、游戏安装信息及相关文件。离线档案包括你输入的玩家名、由玩家名生成的离线 UUID，以及你选择填写的备注。这些档案用于本地选择身份和启动游戏，不代表 Microsoft 账号认证。</p><p>本地游戏及组件可能生成日志，其中可能包含用户名、设备路径、游戏信息或第三方组件输出。请在分享日志之前检查并删除敏感内容。删除游戏或卸载启动器不一定会清除所有独立游戏目录、日志、备份或遗留配置，请按需要手动检查。</p>
<h2>3. 下载、搜索与第三方连接</h2><p>查询版本、安装游戏、搜索或更新模组等操作会连接相应服务，例如 Minecraft 官方服务、BMCLAPI、Modrinth、模组加载器服务或你配置的下载源。接收请求的服务会获得连接所需的网络信息，例如 IP 地址，以及完成操作所需的搜索词、版本、文件标识或文件哈希。实际发送内容取决于你使用的功能和版本。</p><p>第三方服务按其自己的规则处理数据。你可以在使用前查看 <a href="https://www.microsoft.com/privacy/privacystatement">Microsoft 隐私声明</a>、<a href="https://modrinth.com/legal/privacy">Modrinth 隐私政策</a>，以及所选镜像或下载源的说明。第三方模组和游戏服务器也可能独立处理数据，不受本声明控制。</p>
<h2>4. Microsoft 账号登录</h2><p>当前版本尚未提供 Microsoft 账号登录，网站也不接收 Microsoft 密码、访问令牌或刷新令牌。发布者域验证文件中的应用 ID 是公开标识，不包含账号凭据。</p><p>Microsoft 登录上线前，我们将更新本声明，说明授权范围、账号数据、令牌存储、退出与撤销授权方式。请勿通过邮件或 GitHub Issues 向维护者发送密码或令牌。</p>
<h2>5. 主动联系与公开反馈</h2><p>如果你发送邮件，维护者会收到你的邮箱地址、邮件内容和附件，用于回复及处理问题。若你在 GitHub 提交 Issue 或其他公开内容，相关信息通常公开可见，并受 GitHub 的条款和隐私政策约束。</p><p>支持沟通将按解决问题及履行适用法律义务的需要保留。你可以通过联系邮箱申请查询或删除维护者持有的相关沟通记录。第三方平台和托管服务所保存的记录需要依其相应渠道处理。</p>
<h2>6. 你的选择与权利</h2><p>你可以停止访问网站、停止使用启动器、更换下载源、删除本地玩家档案和相关文件，或不提交可选反馈。根据所在地法律，你可能享有访问、更正、删除、限制处理、反对处理或投诉等权利。请通过联系邮箱提出请求，我们会依适用法律处理，并在必要时核实请求人的身份。</p><p>网站及相关第三方基础设施可能在你所在国家或地区以外处理数据。当前网站不提供面向儿童的注册或资料收集功能。监护人如认为儿童通过支持渠道提交了不应保留的个人信息，可联系维护者处理。</p>
<h2>7. 声明更新</h2><p>本声明反映当前网站及现有功能。上线账号登录、遥测、云同步或其他涉及不同数据处理的功能前，我们会更新相关说明，并在适用法律要求时取得同意。本页会标明最近更新日期。</p>'''

ROOT.mkdir(parents=True, exist_ok=True)
(ROOT / 'index.html').write_text(page('网站建设中', 'CobbleLauncher 是面向 Minecraft: Java Edition 的开源启动器。官方网站建设中。', home, '/'))
for slug, title, content in [('terms', '服务条款', terms), ('privacy', '隐私声明', privacy)]:
    body = f'<main class="document"><p class="eyebrow">CobbleLauncher / {slug}</p><h1>{title}</h1><p class="meta">生效及最近更新日期：2026 年 10 月 22 日</p>{content}<a class="back" href="/">← 返回首页</a></main>'
    (ROOT / f'{slug}.html').write_text(page(title, f'CobbleLauncher {title}，说明官方网站及现有启动器功能的相关安排。', body, '/' + slug))
(ROOT / '404.html').write_text(page('页面未找到', '该页面不存在。', '<main class="document"><h1>这里还没有方块。</h1><p>你访问的页面不存在。</p><a class="back" href="/">← 返回首页</a></main>', '/404'))
print('Built static site: index, terms, privacy, 404. Verification JSON preserved.')
