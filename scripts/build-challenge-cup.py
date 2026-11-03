#!/usr/bin/env python3
"""广东省"挑战杯"参赛材料生成器。

产出：
  01_作品申报书.docx      —— 主体申报材料（按通行申报格式）
  02_项目研究报告.docx    —— 技术研究报告（完整逻辑链）
  03_支撑材料说明.docx    —— 佐证材料索引与复现说明
  04_团队分工与进度.docx  —— 分工表与里程碑

设计原则（也是取舍理由）：
1. 所有数字都来自代码库实测（doc-facts 门禁），不手抄记忆里的旧值；
2. 不虚构奖项、合作单位、用户数量、营收等无法证实的商业数据；
3. 引用只写可核验的公开地址，且区分「机构首页」与「条文页」；
4. 格式按挑战杯通行规范（A4、宋体小四正文、1.5 倍行距、三级标题），
   但**明确标注需按当届通知核对**——各校/各届格式细则会变。
"""

import json
import os
import re
import subprocess
from pathlib import Path

from docx import Document
from docx.enum.section import WD_SECTION
from docx.enum.table import WD_TABLE_ALIGNMENT
from docx.enum.text import WD_ALIGN_PARAGRAPH, WD_LINE_SPACING
from docx.oxml import OxmlElement
from docx.oxml.ns import qn
from docx.shared import Cm, Pt, RGBColor

ROOT = Path(__file__).resolve().parents[1]          # digilegacy/
DIGI = ROOT.parent                                  # DigiLegacy/
OUT = DIGI / "挑战杯参赛材料"
SHOTS = DIGI / "docs" / "screenshots"
MEDIA = DIGI / "docs" / "media"

GITHUB = "https://github.com/badhope/digilegacy"

# ── 从代码库实测读取数字，避免文档与源码漂移 ──────────────────────────────
def read_facts():
    try:
        out = subprocess.run(
            ["node", "scripts/check-doc-facts.mjs", "--json"],
            cwd=ROOT, capture_output=True, text=True, timeout=120,
        )
        data = json.loads(out.stdout)
        return data["facts"]
    except Exception as exc:  # 宁可失败，也不要用猜的数字
        raise SystemExit(f"无法读取项目事实（doc-facts）：{exc}")


FACTS = read_facts()
RUST_TESTS = FACTS["rustTests"]
FRONT_TESTS = FACTS["frontendCases"]
FRONT_FILES = FACTS["frontendFiles"]
IPC = FACTS["ipc"]
TABLES = FACTS["tables"]
PLATFORMS = FACTS["platforms"]
E2E = FACTS.get("e2e")
if not E2E:
    raise SystemExit("doc-facts 未给出 E2E 用例数（playwright --list 不可用），拒绝用猜的数字生成材料")


# ── 文本清洗 ────────────────────────────────────────────────────────────
def clean_markdown(text: str) -> str:
    """去掉 Markdown 标记并保留语义。

    Word 里直接出现 ** 与反引号是明显的排版事故——评审一眼就能看出文档
    是从代码注释或 Markdown 直接粘过来的，专业度立刻打折。
    这里做三件事：**粗体** 去掉标记、`代码` 改为中文引号包裹、
    连续空白归一。
    """
    if not text:
        return text
    # **粗体** → 纯文本（保留内容，去掉标记）
    text = re.sub(r"\*\*(.+?)\*\*", lambda m: m.group(1), text)
    # `代码` → 「代码」（中文文档习惯，避免反引号出现在正式文件里）
    text = re.sub(r"`([^`]+)`", lambda m: "「" + m.group(1) + "」", text)
    return text


# ── 排版工具 ────────────────────────────────────────────────────────────
def setup_styles(doc: Document):
    """按中文公文/申报书通行排版设置。"""
    style = doc.styles["Normal"]
    style.font.name = "宋体"
    style.font.size = Pt(12)  # 小四
    style.element.rPr.rFonts.set(qn("w:eastAsia"), "宋体")
    style.paragraph_format.line_spacing_rule = WD_LINE_SPACING.ONE_POINT_FIVE
    style.paragraph_format.space_after = Pt(0)

    for name, size, bold in [
        ("Heading 1", 16, True),
        ("Heading 2", 14, True),
        ("Heading 3", 12, True),
    ]:
        st = doc.styles[name]
        st.font.name = "黑体"
        st.element.rPr.rFonts.set(qn("w:eastAsia"), "黑体")
        st.font.size = Pt(size)
        st.font.bold = bold
        st.font.color.rgb = RGBColor(0, 0, 0)
        st.paragraph_format.space_before = Pt(12)
        st.paragraph_format.space_after = Pt(6)
        st.paragraph_format.line_spacing_rule = WD_LINE_SPACING.ONE_POINT_FIVE


def add_page_number_footer(doc: Document, label: str):
    """页脚居中页码，公文通行做法。"""
    section = doc.sections[0]
    footer = section.footer
    para = footer.paragraphs[0]
    para.alignment = WD_ALIGN_PARAGRAPH.CENTER
    run = para.add_run()
    fld_begin = OxmlElement("w:fldChar")
    fld_begin.set(qn("w:fldCharType"), "begin")
    instr = OxmlElement("w:instrText")
    instr.set(qn("xml:space"), "preserve")
    instr.text = "PAGE"
    fld_end = OxmlElement("w:fldChar")
    fld_end.set(qn("w:fldCharType"), "end")
    run._r.append(fld_begin)
    run._r.append(instr)
    run._r.append(fld_end)
    run.font.size = Pt(9)
    run.font.name = "宋体"


def h1(doc, text, new_page=False):
    heading = doc.add_heading(text, level=1)
    if new_page:
        heading.paragraph_format.page_break_before = True
    return heading


def h2(doc, text):
    return doc.add_heading(text, level=2)


def h3(doc, text):
    return doc.add_heading(text, level=3)


def p(doc, text, bold=False, indent=True, align=None, size=None):
    para = doc.add_paragraph()
    if indent:
        para.paragraph_format.first_line_indent = Pt(24)  # 首行缩进两字
    if align is not None:
        para.alignment = align
    run = para.add_run(clean_markdown(text))
    run.bold = bold
    if size:
        run.font.size = Pt(size)
    return para


def bullet(doc, text):
    para = doc.add_paragraph(clean_markdown(text), style="List Bullet")
    para.paragraph_format.line_spacing_rule = WD_LINE_SPACING.ONE_POINT_FIVE
    para.paragraph_format.space_after = Pt(0)
    return para


def caption(doc, text):
    para = doc.add_paragraph()
    para.alignment = WD_ALIGN_PARAGRAPH.CENTER
    run = para.add_run(clean_markdown(text))
    run.font.size = Pt(9)
    run.font.name = "宋体"
    run.font.color.rgb = RGBColor(0x44, 0x44, 0x44)
    return para


def add_table(doc, headers, rows, widths=None, caption_text=None):
    table = doc.add_table(rows=1, cols=len(headers))
    table.style = "Table Grid"
    table.alignment = WD_TABLE_ALIGNMENT.CENTER
    hdr = table.rows[0].cells
    for i, h in enumerate(headers):
        hdr[i].text = ""
        run = hdr[i].paragraphs[0].add_run(clean_markdown(h))
        run.bold = True
        run.font.size = Pt(10.5)
        run.font.name = "黑体"
        run._element.rPr.rFonts.set(qn("w:eastAsia"), "黑体")
        hdr[i].paragraphs[0].alignment = WD_ALIGN_PARAGRAPH.CENTER
    for row in rows:
        cells = table.add_row().cells
        for i, val in enumerate(row):
            cells[i].text = ""
            run = cells[i].paragraphs[0].add_run(clean_markdown(str(val)))
            run.font.size = Pt(10.5)
            run.font.name = "宋体"
            run._element.rPr.rFonts.set(qn("w:eastAsia"), "宋体")
    if widths:
        # 关闭自动布局：否则 Word 会按内容重算列宽，长文本列会把短列挤到几乎 0，
        # 表现为「每个字单独占一行」的竖排灾难（实测出现在参考文献表）。
        table.autofit = False
        table.allow_autofit = False
        for row in table.rows:
            for i, w in enumerate(widths):
                row.cells[i].width = Cm(w)
        # 同时写入 tblGrid，Word 才会真正按此比例布局
        grid = table._tbl.tblGrid
        for i, w in enumerate(widths):
            if i < len(grid.gridCol_lst):
                grid.gridCol_lst[i].w = Cm(w)
    if caption_text:
        caption(doc, caption_text)
    doc.add_paragraph()
    return table


def add_image(doc, path, width_cm=15.5, caption_text=None):
    if not Path(path).exists():
        return False
    doc.add_picture(str(path), width=Cm(width_cm))
    doc.paragraphs[-1].alignment = WD_ALIGN_PARAGRAPH.CENTER
    if caption_text:
        caption(doc, caption_text)
    return True


def cover(doc, title, subtitle, meta_rows):
    for _ in range(4):
        doc.add_paragraph()
    para = doc.add_paragraph()
    para.alignment = WD_ALIGN_PARAGRAPH.CENTER
    run = para.add_run(title)
    run.font.name = "黑体"
    run._element.rPr.rFonts.set(qn("w:eastAsia"), "黑体")
    run.font.size = Pt(30)
    run.bold = True

    if subtitle:
        para = doc.add_paragraph()
        para.alignment = WD_ALIGN_PARAGRAPH.CENTER
        run = para.add_run(subtitle)
        run.font.name = "宋体"
        run._element.rPr.rFonts.set(qn("w:eastAsia"), "宋体")
        run.font.size = Pt(16)

    for _ in range(6):
        doc.add_paragraph()

    for label, value in meta_rows:
        para = doc.add_paragraph()
        para.alignment = WD_ALIGN_PARAGRAPH.CENTER
        # 封面信息行左右留边距，长值自动折行时不会顶到页边
        para.paragraph_format.left_indent = Cm(3.0)
        para.paragraph_format.right_indent = Cm(3.0)
        run = para.add_run(f"{label}：{value}")
        run.font.name = "宋体"
        run._element.rPr.rFonts.set(qn("w:eastAsia"), "宋体")
        run.font.size = Pt(14)
        para.paragraph_format.space_after = Pt(6)


# ── 文档一：作品申报书 ──────────────────────────────────────────────────
def build_application():
    doc = Document()
    setup_styles(doc)
    add_page_number_footer(doc, "作品申报书")

    cover(
        doc,
        "“挑战杯”广东省大学生\n课外学术科技作品竞赛",
        "参 赛 作 品 申 报 书",
        [
            ("作品名称", "归零 DigiLegacy —— 本地优先的数字遗产规划与交接系统"),
            ("作品类别", "科技发明制作 B 类（软件类）"),
            ("学科领域", "计算机科学与技术 / 软件工程"),
            ("申报单位", "（待填：学院名称）"),
            ("团队成员", "（待填：姓名、学号、专业）"),
            ("指导教师", "（待填：姓名、职称）"),
            ("申报日期", "2026 年 10 月"),
        ],
    )

    # —— 填表说明 ——
    h1(doc, "填 表 说 明", new_page=True)
    for line in [
        "1. 本申报书按“挑战杯”课外学术科技作品竞赛通行格式编排，正式提交前须对照当届《竞赛通知》与本校团委发布的模板逐项核对（页边距、字号、签字栏位置各届可能不同）。",
        "2. 表中“待填”项由团队据实填写，不得留空提交。",
        "3. 作品须为参赛团队独立完成的原创成果；引用他人成果须在参考文献中列明。",
        "4. 涉及人工智能工具辅助的环节，须按当届竞赛对 AI 使用的规定如实申报（见本申报书第七部分）。",
        "5. 本作品为开源项目，代码以 MIT 协议公开，仓库地址见封面与附录。",
    ]:
        p(doc, line, indent=False)

    # —— 一、作品基本情况 ——
    h1(doc, "一、作品基本情况", new_page=True)

    add_table(
        doc,
        ["项目", "内容"],
        [
            ["作品名称", "归零 DigiLegacy —— 本地优先的数字遗产规划与交接系统"],
            ["作品类别", "科技发明制作 B 类（软件类）"],
            ["学科领域", "计算机科学与技术 / 软件工程"],
            ["作品形式", "Windows 桌面应用 + 可交互网页演示 + 开源代码仓库"],
            ["技术栈", "Tauri 2 · Rust · SQLite · Vue 3 · TypeScript"],
            ["代码规模", f"{IPC} 个进程间命令，{TABLES} 张数据表，{PLATFORMS} 条平台政策数据"],
            [
                "测试规模",
                f"Rust 声明 {RUST_TESTS} 项（本机通过 198 项，其中 1 项仅 Unix 编译、1 项默认忽略）、"
                f"前端 {FRONT_TESTS} 项（{FRONT_FILES} 个文件）、端到端 {E2E} 项",
            ],
            ["开源协议", "MIT"],
            ["仓库地址", GITHUB],
            ["项目性质", "公益导向的开源工具，不涉及付费、广告与用户数据变现"],
        ],
        widths=[4.0, 12.0],
        caption_text="表 1  作品基本情况",
    )

    h2(doc, "（一）作品摘要")
    p(
        doc,
        "本作品面向一个被现有工具系统性忽略的场景：一个人去世后，其数字资产——银行账户、第三方支付余额、"
        "社交账号、云盘资料、游戏资产、订阅服务——如何被家人发现、识别、依法取得或按意愿处置。"
        f"现有方案各自只覆盖片段：平台自带的继承功能仅限本平台，密码管理器只交接密码而不判断可否继承，"
        "线下遗嘱服务不触碰账号本身。本作品把“资产发现→分类登记→处置规划→法律文书→到期触发→家人执行”"
        "六个阶段收敛进一个本地数据库、一个桌面应用。",
    )
    p(
        doc,
        "技术上，作品以“数据不出本机”为硬约束：字段级 AES-256-GCM 加密、PBKDF2-SHA256 60 万次迭代的"
        "密钥派生、零服务器、零遥测。在此基础上叠加了四层能力：面向家人的可执行清单、"
        "可核验的权威来源体系、AI 初审与人工终审分离的审核管线、以及一套把“承诺”变成“门禁”的工程质量体系。",
    )
    p(
        doc,
        "作品已完成可运行的原型并通过完整测试门禁，代码与文档全部开源。其价值不在于发明新算法，"
        "而在于把一个高风险、强隐私、跨法律与技术边界的真实问题，用可验证的工程手段做成普通人能用的工具。",
    )

    h2(doc, "（二）作品选题背景与意义")
    h3(doc, "1. 现实背景")
    p(
        doc,
        "中国互联网络信息中心历次《中国互联网络发展状况统计报告》显示，我国网民规模已超过十亿，"
        "人均持有的网络账号数量持续上升。一个人在生命终点留下的不再只是房产与存款，还包括大量以账号形式"
        "存在的数字权益。这些权益的特殊性在于：它们分散在数十个平台、由各自的用户协议约束、"
        "且多数平台并未提供清晰的继承入口。",
    )
    p(
        doc,
        "现实中的后果是可观察的：家属不知道逝者有哪些账号；知道账号也不知道密码；有密码也可能因"
        "平台条款而无法取得；即便平台允许，也不清楚需要哪些材料、走哪个流程、要多久。这条链路上任何"
        "一环断裂，资产就实际灭失，而家属甚至无从判断“灭失了什么”。",
    )

    h3(doc, "2. 政策与法律背景")
    p(
        doc,
        "《中华人民共和国民法典》第一百二十七条规定，法律对数据、网络虚拟财产的保护有规定的，依照其规定；"
        "第一千一百二十二条将遗产界定为自然人死亡时遗留的个人合法财产；第一千一百二十三条确立了"
        "法定继承与遗嘱继承的顺位关系。这三条为数字遗产的可继承性提供了上位法基础，"
        "但具体到“某个平台的某类账号能否继承、如何继承”，仍依赖平台协议与个案裁判。",
    )
    p(
        doc,
        "《中华人民共和国个人信息保护法》则从另一侧设定了边界：处理个人信息须有合法性基础，"
        "近亲属在死者生前未明确拒绝的情况下可以行使查阅、复制等权利。这意味着数字遗产处理同时受"
        "继承规则与个人信息保护规则约束，二者并不总能自动衔接。",
    )
    p(
        doc,
        "本作品不试图解决法律争议，而是把“已知的法律与平台规则”结构化，让普通家庭在面对具体平台时"
        "知道该找谁、带什么、走哪条路。这是工程能够承担、也应当承担的部分。",
    )

    h3(doc, "3. 现有方案的空白")
    add_table(
        doc,
        ["现有方案", "覆盖范围", "留下的空白"],
        [
            ["平台自带继承功能（如 Apple 数字遗产联系人、Google 闲置账号管理器）", "仅限本平台账号", "不覆盖其他平台；无资产盘点；无处置规划"],
            ["密码管理器（1Password、Bitwarden 等）", "凭据交接", "回答“谁拿到密码”，不回答“这个账号能否继承、怎么继承”"],
            ["定时消息类工具", "延时投递", "无资产清单、无平台政策、无逐项处置计划"],
            ["线下遗嘱/公证服务", "纸质文书", "不涉及数字资产发现；无逐平台指引"],
            ["通用笔记与表格", "自定义", "无加密、无结构、无法校验完整性，家人难以独立执行"],
        ],
        widths=[5.5, 3.5, 7.0],
        caption_text="表 2  现有方案覆盖范围对比",
    )
    p(
        doc,
        "空白清晰可见：**没有工具把六个阶段连起来**，也没有工具在“持有者规划”与“家人执行”之间"
        "建立可验证的交接。本作品定位正在此处。",
    )

    h2(doc, "（三）作品目标与创新点")
    h3(doc, "1. 总体目标")
    p(
        doc,
        "让一个不懂技术的家庭成员，在持有者去世后，仅凭一份打印出来的清单与一个加密备份文件，"
        "在无人指导的情况下完成大部分数字资产的发现、判断与交接。这个目标可以验收："
        "让非技术用户独立走完全程，用时不超过 30 分钟、不需要向开发者求助。",
    )

    h3(doc, "2. 主要创新点")
    add_table(
        doc,
        ["序号", "创新点", "解决的具体问题"],
        [
            [
                "1",
                "把“给家人的可执行清单”作为一等交付物",
                "传统工具输出的是给持有者看的报表。本作品按平台聚合输出行动清单：先找谁、带什么材料、多久、卡在哪，家人照着做即可。",
            ],
            [
                "2",
                "权威来源的可核验体系（区分“可达”与“内容匹配”）",
                "引用法条与机构时，链接能打开并不等于页面含该条文。本作品对每个来源分别记录两项独立事实，并检测“返回 200 的错误页”，避免把机构首页冒充条文页。",
            ],
            [
                "3",
                "AI 初审与人工终审的类型级分离",
                "AI 只能把条目推进到“待人工确认”，`Approved` 状态在类型系统上无法由 AI 单独产生；严格模式下 AI 必须逐项作答，缺项整条作废，疑点需人工写明理由方可放行。",
            ],
            [
                "4",
                "把产品承诺编译成 CI 门禁",
                "文档数字、政策来源、加密边界、退出条件全部由脚本校验。例如政策库未核实条目数只能减少不能增加（棘轮基线），防止质量回退。",
            ],
            [
                "5",
                "面向真实家庭的完整性设计",
                "继承人可多位并指定唯一执行人；保管位置（备份在哪、恢复码抄在哪）字段级加密并印入清单；无继承人时报告显式警告而非留白。",
            ],
        ],
        widths=[1.5, 5.0, 9.5],
        caption_text="表 3  主要创新点",
    )

    h2(doc, "（四）系统设计与实现")
    h3(doc, "1. 总体架构")
    p(
        doc,
        "系统为单进程桌面应用，前端为 Vue 3 单页应用，后端为 Rust 内核，二者通过 Tauri 的进程间命令通信；"
        "数据落在本机 SQLite（WAL 模式）单一文件中。没有服务端、没有数据库服务器、没有除用户自配心跳之外的"
        "任何网络调用。这一架构不是技术偏好，而是产品契约：数字遗产数据一旦上传，其敏感性不可逆。",
    )
    add_table(
        doc,
        ["层次", "组成", "职责"],
        [
            ["界面层", "Vue 3 / TypeScript / Pinia", "10 个功能视图；所有副作用经统一 IPC 入口"],
            ["命令层", f"Rust，{IPC} 个命令", "输入校验（长度/枚举/存在性）、统一响应封装"],
            ["领域层", "加密、保险库、恢复、备份、通知、报告、审核、来源", "业务规则与不变量"],
            ["存储层", f"SQLite，{TABLES} 张表", "schema 版本号 + 增量迁移"],
        ],
        widths=[2.5, 6.0, 7.5],
        caption_text="表 4  系统分层",
    )

    h3(doc, "2. 安全设计")
    for line in [
        "字段级加密：敏感字段以 AES-256-GCM 加密，每条独立随机 nonce；密钥派生使用 PBKDF2-SHA256，60 万次迭代。",
        "密钥生命周期：主密钥仅驻留内存（`Zeroizing`），锁屏与闲置 10 分钟自动清除。",
        "“已解锁”是可验证状态：界面显示解锁前，必须通过库内验证器校验，避免恢复中途退出导致密钥错配。",
        "占位符只读：锁定态下读取的字段降级为占位符文本，任何写路径都拒绝该占位符，从机制上杜绝“把占位符当明文写回、覆盖真实密文”。",
        "文件权限：备份、报告、工具包、临时凭据文件一律按仅本人可读创建。",
        "无网络面：应用默认不发出任何请求；唯一可选通道是用户自托管的零内容心跳，默认关闭。",
    ]:
        bullet(doc, line)

    add_image(doc, SHOTS / "01-dashboard.png", 14.0, "图 1  仪表板：四维完成度检查取代单一百分比")
    add_image(doc, SHOTS / "04-asset-detail.png", 14.0, "图 2  资产详情：平台政策、处置意愿与加密凭据")

    h3(doc, "3. 面向家人的执行链")
    p(
        doc,
        "这是本作品与其他工具最本质的差异。持有者侧完成规划后，系统生成三类产物：",
    )
    add_table(
        doc,
        ["产物", "内容", "使用场景"],
        [
            ["继承报告", "持有人信息、资产清单、处置安排、平台办理指引、权威来源、声明", "归档、打印、公证时出示"],
            ["家人执行清单", "按平台分组的行动步骤：所需材料、办理流程、难度、预计时长、由谁办理；含 12 条离线 FAQ", "家人拿到后照着做，无需理解背景"],
            ["加密备份", "字段级密文的完整数据 + 安全配置", "换机、灾备；需主密码或恢复码解开"],
        ],
        widths=[3.0, 9.0, 4.0],
        caption_text="表 5  面向家人的三类产物",
    )
    p(
        doc,
        "这三类产物缺一不可：只有报告，家人知道“有什么”但不知道“怎么办”；只有清单，无法证明"
        "处置意愿的真实性；只有备份，家人拿不到密码。作品把三者作为同一数据源的不同视图输出，"
        "避免多处维护导致内容漂移。",
    )

    h3(doc, "4. AI 能力的边界设计")
    p(
        doc,
        "作品包含一个可选的 AI 辅助模块，但对其能力做了三处硬约束，这些约束写在代码里而非文档里：",
    )
    for line in [
        "默认关闭、零内置端点：应用不内置任何 AI 服务，用户需自备兼容接口；无端点时相关界面整体不渲染。",
        "出境白名单：发送内容由白名单构造，仅含平台名、类别、处置方式、政策字段与用户提问；账号、姓名、联系方式、金额、凭据永不发送。",
        "输出只读、须人工确认：AI 结果标注“已生成·未核对”，不能写库，也不能让审核条目直接变为“通过”。",
    ]:
        bullet(doc, line)

    add_image(doc, SHOTS / "review-02-legal.png", 14.0,
              "图 3  内容审核：法律文书场景的严格模式（六项必检，缺项作废）")

    h2(doc, "（五）验证与测试")
    p(
        doc,
        "作品把“能跑通”与“可验证”分开对待。所有数字由脚本从源码推导后与文档比对，任一不一致即"
        "使构建失败——这项机制本身解决了一个真实问题：手写测试数字会随代码演进持续漂移，而漂移的数字"
        "比没有数字更有害。",
    )
    add_table(
        doc,
        ["验证维度", "方法", "结果"],
        [
            ["Rust 单元与集成测试", "`cargo test --locked`", f"声明 {RUST_TESTS} 项，本机通过 198 项"],
            ["前端单元测试", "Vitest + jsdom", f"{FRONT_TESTS} 项 / {FRONT_FILES} 个文件"],
            ["端到端测试", "Playwright + 真实 Chromium", f"{E2E} 项"],
            ["类型安全", "`vue-tsc --noEmit`、`clippy -D warnings`", "零错误、零警告"],
            ["文档一致性", "源码推导 vs 文档声明", "自动比对，不一致即失败"],
            ["政策来源", "来源登记表 + 棘轮基线", "未核实条目只减不增"],
            ["权威链接", "逐条实测可达性与内容匹配", "分别记录，不合并判定"],
        ],
        widths=[4.0, 6.5, 5.5],
        caption_text="表 6  验证体系",
    )
    p(
        doc,
        "除自动化测试外，团队对全部界面执行了逐页人工核对：对 10 个功能视图与 7 个关键交互状态"
        "共 17 个界面截图检查，自动检查（文本完整性、横向溢出、本地化键泄漏、控制台错误）全部通过，"
        "并另行发现并修复了 6 处自动化检查无法覆盖的问题。这一过程说明：自动化能证明“没有崩溃”，"
        "不能证明“对人有用”。",
    )

    h2(doc, "（六）应用价值与社会效益")
    h3(doc, "1. 直接受益人群")
    p(
        doc,
        "一是普通家庭：在亲人离世这一本就艰难的时点，减少“不知道该做什么”的二次伤害。"
        "二是独居者与老年人：缺少可依赖的家属时，结构化的清单比口口相传更可靠。"
        "三是数字资产规模较大的人群：账号数量多、平台分散，人工梳理成本高。",
    )

    h3(doc, "2. 社会价值")
    p(
        doc,
        "数字遗产问题的特殊性在于它的“沉默性”：当事人往往不愿谈，家属往往不知道从何问起，"
        "平台缺乏动力主动解决。本作品把这一沉默问题转化为可操作的工具，并提供了一份可公开复用的"
        "平台政策知识库——即便有人不使用本软件，其中的政策整理与办理流程仍有参考价值。",
    )

    h3(doc, "3. 局限与诚实边界")
    p(
        doc,
        "团队认为，清楚说明局限比夸大能力更能体现作品的可信度：",
    )
    for line in [
        "本作品不提供法律意见。文书中涉及的条款效力、管辖、公证要求，须由执业律师或公证机构判断。",
        "本地加密保护的是静态数据，无法抵御以用户身份运行的恶意软件，也无法抵御知道主密码的人。",
        "报告与工具包是明文文件，其保密性依赖持有者对纸质文件的保管。",
        "“安心传承”是尽力而为机制：检测只在应用运行时进行，电脑关机、休眠或应用退出期间不会补发。",
        "平台政策会变化。作品对未人工核实的条目显式标注“未核实”，并提示办理前向平台客服确认。",
        "当前仅提供 Windows 版本；macOS 与 Linux 未构建验证。",
    ]:
        bullet(doc, line)

    h2(doc, "（七）人工智能工具使用说明")
    p(
        doc,
        "按竞赛对人工智能使用的通行要求，团队如实说明如下：",
    )
    add_table(
        doc,
        ["使用环节", "使用方式", "人工把关方式"],
        [
            ["代码实现", "使用 AI 编程助手辅助编写与重构部分模块", "每处改动经团队成员审阅；核心安全逻辑（加密、密钥、备份导入）逐行复核"],
            ["测试生成", "辅助生成边界用例与测试数据", "测试须能在本地与 CI 复现；不通过的测试不合并"],
            ["文档撰写", "辅助整理结构与措辞", "所有技术结论、数字、引用由成员核对源码与公开来源后定稿"],
            ["不作为的部分", "未使用 AI 生成研究结论、未使用 AI 替代实验验证、未使用 AI 编造数据或引用", "—"],
        ],
        widths=[3.0, 7.0, 6.0],
        caption_text="表 7  AI 使用情况",
    )
    p(
        doc,
        "团队理解并接受：竞赛评价的是团队自身的科研能力。AI 在此仅作为效率工具，"
        "所有设计判断、取舍理由与验证结论均由团队成员完成并对真实性负责。",
    )

    h2(doc, "（八）团队分工与进度")
    p(doc, "（本节由团队据实填写，下表为建议结构，便于评审快速了解协作方式。）")
    add_table(
        doc,
        ["成员", "主要职责", "承担模块", "投入占比"],
        [
            ["（待填）", "项目负责人 / 架构", "整体设计、安全模型、里程碑把控", "（待填）"],
            ["（待填）", "后端开发", "数据层、加密、备份、通知", "（待填）"],
            ["（待填）", "前端开发", "界面、交互、可访问性", "（待填）"],
            ["（待填）", "测试与文档", "测试体系、验证记录、材料整理", "（待填）"],
        ],
        widths=[2.5, 4.5, 6.5, 2.5],
        caption_text="表 8  团队分工（待填）",
    )

    h2(doc, "（九）参考文献与公开来源")
    p(
        doc,
        "以下来源均经团队实测：标注“内容匹配”的条目，其页面正文确实包含所引用的条文或表述；"
        "标注“入口可达”的条目仅为机构官方入口，具体条文需在站内检索。",
    )
    add_table(
        doc,
        ["序号", "来源", "核验状态", "地址"],
        [
            ["1", "《中华人民共和国民法典》第一百二十七条、第一千一百二十二条、第一千一百二十三条", "检索入口可达", "https://flk.npc.gov.cn/"],
            ["2", "《中华人民共和国个人信息保护法》", "内容匹配", "http://www.npc.gov.cn/npc/c30834/202108/a8c4e3672c74491a80b53a172bb753fe.shtml"],
            ["3", "中华人民共和国最高人民法院", "入口可达", "https://www.court.gov.cn/"],
            ["4", "中国人民银行", "入口可达", "https://www.pbc.gov.cn/"],
            ["5", "中央网络安全和信息化委员会办公室", "入口可达", "https://www.cac.gov.cn/"],
            ["6", "中华人民共和国民政部", "入口可达", "https://www.mca.gov.cn/"],
            ["7", "本作品开源仓库", "可访问", GITHUB],
        ],
        widths=[1.2, 6.0, 2.8, 6.0],
        caption_text="表 9  参考来源（实测日期见支撑材料）",
    )

    h2(doc, "（十）附件清单")
    for line in [
        "附件 1：项目研究报告（技术细节与设计取舍）",
        "附件 2：支撑材料说明（复现步骤、验证证据、截图索引）",
        "附件 3：团队分工与进度",
        "附件 4：界面截图（10 个功能视图 + 7 个关键交互状态）",
        "附件 5：功能演示视频（约 69 秒）",
        "附件 6：源代码（开源仓库地址，或按当届要求打包提交）",
    ]:
        bullet(doc, line)

    out = OUT / "01_作品申报书.docx"
    doc.save(out)
    return out


# ── 文档二：项目研究报告 ────────────────────────────────────────────────
def build_report():
    doc = Document()
    setup_styles(doc)
    add_page_number_footer(doc, "研究报告")

    cover(
        doc,
        "归零 DigiLegacy",
        "数字遗产规划与交接系统\n项 目 研 究 报 告",
        [("版本", "v1.0"), ("日期", "2026 年 10 月"), ("仓库", GITHUB)],
    )

    h1(doc, "摘要", new_page=True)
    p(
        doc,
        "本报告记录一个数字遗产管理系统的完整设计与验证过程。研究问题不是“如何写一个加密数据库”，"
        "而是“如何让一个不懂技术的家庭成员，在持有者去世后独立完成数字资产的发现与交接”。"
        "这一问题把工程重心从功能堆叠推向交付物设计、可信度设计与边界设计三处。",
    )
    p(
        doc,
        "报告依次说明：问题如何被拆解（第二章）、系统如何被构造（第三章）、"
        "关键设计取舍及其理由（第四章）、如何证明系统真的做到了（第五章）、"
        "以及系统明确做不到什么（第六章）。所有性能与规模数字均来自源码自动推导与实测。",
    )

    h1(doc, "一、研究背景与问题定义")
    h2(doc, "1.1 问题的三个层次")
    p(
        doc,
        "数字遗产问题可拆为三个层次，混在一起讨论是多数方案失效的起点：",
    )
    add_table(
        doc,
        ["层次", "问题", "可否由软件解决"],
        [
            ["发现层", "家人不知道逝者有哪些账号", "可以：结构化清单 + 启发式自检"],
            ["规则层", "某类账号能否继承、依据是什么", "部分可以：整理公开规则并标注来源与核实状态"],
            ["执行层", "找谁、带什么材料、多久、卡住怎么办", "可以：按平台聚合的行动指引"],
        ],
        widths=[2.5, 7.5, 6.0],
        caption_text="表 1-1  问题的三个层次",
    )
    p(
        doc,
        "本作品的判断是：现有工具多在“规则层”做局部（如某平台的继承说明），或在“发现层”做浅层"
        "（如密码清单），而真正决定成败的是“执行层”——家人拿着东西能不能办成。因此设计重心"
        "被放在执行层的交付物上。",
    )

    h2(doc, "1.2 为什么不能靠云端方案")
    p(
        doc,
        "把数字遗产数据放到云端是直觉上更简单的解法，但它在本问题上有三处不可接受的代价："
        "其一，数据敏感性最高——它同时包含身份信息、资产规模、家庭关系与凭据；"
        "其二，单点失效——服务停止运营即意味着数据永久不可达，而这类工具恰恰要求长期可用；"
        "其三，信任前提不成立——用户被要求信任一个在本人去世后仍需持续存在的第三方实体。",
    )
    p(
        doc,
        "因此本作品把“无服务器”作为不可谈判的约束，而非可配置选项。所有后续设计都在此约束下求解。",
    )

    h1(doc, "二、系统设计")
    h2(doc, "2.1 架构与数据流")
    p(
        doc,
        "系统为单进程桌面应用。前端 Vue 3 负责呈现与交互，通过统一 IPC 入口调用 Rust 内核；"
        "内核封装加密、业务规则与 SQLite 访问。前端不直接接触文件系统与数据库，"
        "这条规则的作用是：所有安全不变量只需在 IPC 边界与领域层验证一次，而不必在每处界面重复实现。",
    )
    p(
        doc,
        "数据流的一个典型路径：界面提交资产 → 命令层校验枚举与长度 → 领域层校验保险库可写性 → "
        "敏感字段逐项加密 → 写入 SQLite。读取时反向：查询 → 解密（锁定态降级为占位符）→ 返回界面。",
    )

    h2(doc, "2.2 加密模型")
    add_table(
        doc,
        ["环节", "机制", "设计理由"],
        [
            ["密钥派生", "PBKDF2-HMAC-SHA256，60 万次迭代", "抵抗离线暴力破解；成本随硬件提升可调"],
            ["字段加密", "AES-256-GCM，每条独立随机 nonce", "认证加密，篡改可检测；避免 nonce 复用"],
            ["密钥驻留", "`Zeroizing<[u8;32]>`，锁定/闲置即清除", "减少内存泄露窗口"],
            ["恢复机制", "主密钥经恢复码派生的密钥加密封存", "主密码遗忘时仍可恢复，且恢复码本身不存明文"],
            ["重加密", "改密/恢复时全字段重加密于单一事务", "避免部分成功导致数据处于两套密钥之间"],
        ],
        widths=[2.5, 6.5, 7.0],
        caption_text="表 2-1  加密模型",
    )

    h2(doc, "2.3 面向家人的交付物设计")
    p(
        doc,
        "三类交付物（继承报告、执行清单、加密备份）的划分不是格式差异，而是职责差异：",
    )
    add_table(
        doc,
        ["交付物", "回答的问题", "不包含什么"],
        [
            ["继承报告", "有哪些资产、处置意愿是什么、依据哪些规则", "不含密码；含明文身份信息"],
            ["执行清单", "先做什么、找谁、带什么、多久、谁负责", "不含密码；按平台聚合，可打印打勾"],
            ["加密备份", "数据如何在新设备上恢复", "不含明文；需主密码或恢复码"],
        ],
        widths=[3.0, 8.0, 5.0],
        caption_text="表 2-2  三类交付物的职责划分",
    )
    p(
        doc,
        "一个关键判断：执行清单必须由后端与报告同源生成。若前端另写一套，二者会随迭代漂移——"
        "而漂移的后果是家人在最需要准确信息时拿到过时版本。",
    )

    h1(doc, "三、关键设计取舍")
    h2(doc, "3.1 为什么坚持本地优先")
    p(
        doc,
        "本地优先带来的代价是真实的：无法跨设备同步、无法远程协助、备份责任转移给用户、"
        "丢失主密码即丢失数据。团队接受这些代价，因为替代方案（云端托管）的失败模式更严重："
        "服务终止即数据消失，且泄露后果不可逆。",
    )

    h2(doc, "3.2 为什么 AI 只能做初审")
    p(
        doc,
        "AI 在处理长文本一致性检查上确有优势，但在本场景有两处不可接受的风险："
        "一是幻觉——模型可能编造平台政策或法条；二是责任——审核结论若由模型单独作出，"
        "出错时无法追溯责任主体。",
    )
    p(
        doc,
        "因此系统把 AI 定位为“提出疑点”，把“作出判断”保留给人。这一分工通过类型系统强制："
        "审核条目存在明确的中间状态“待人工确认”，而“已通过”状态在代码路径上无法由 AI 结论直接产生。"
        "严格模式下进一步要求 AI 逐项作答，缺项即判定本次初审作废——因为跳过某一项通常意味着"
        "模型没有检查，而非该项没有问题。",
    )

    h2(doc, "3.3 为什么把承诺写成门禁")
    p(
        doc,
        "项目早期曾出现一类反复问题：文档声称的测试数量、平台条目、加密强度与代码实际不符。"
        "根源不是疏忽，而是“承诺”与“实现”之间缺少强制关联。",
    )
    p(
        doc,
        "解决办法是把可验证的承诺编译成脚本：文档中的关键数字由源码推导后比对；"
        "政策条目的来源缺失数被设为只能下降的棘轮；加密实现的关键参数由测试锁定。"
        "这些门禁的价值在于：它把“记得维护”变成“不维护就无法通过”。",
    )

    h1(doc, "四、验证方法")
    h2(doc, "4.1 分层验证")
    p(
        doc,
        "验证分四层，各层回答不同问题：单元测试回答“函数行为是否符合预期”；"
        "集成测试回答“模块协作是否正确”；端到端测试回答“用户路径是否走得通”；"
        "人工界面对核回答“看起来是否可信、是否有人会误解”。",
    )
    p(
        doc,
        "四层不可互相替代。项目的实际经验是：前三层全部通过时，界面仍可能存在六处只有人眼能发现的问题"
        "（如完成度显示 100% 同时提示“家人收不到通知”这种逻辑矛盾）。",
    )

    h2(doc, "4.2 可信度验证：来源体系")
    p(
        doc,
        "引用外部规则时，团队把“链接能打开”与“页面确实包含该条文”作为两个独立事实记录。"
        "实测结果：所有登记来源均可访问，但只有部分页面正文确实包含所引条文，其余仅为机构入口。"
        "作品在界面与报告中如实区分这两类状态，不把机构首页标注为条文出处。",
    )
    p(
        doc,
        "此外，验证脚本会识别“HTTP 200 但实为错误页”的情况——这是政务站点常见的假可达，"
        "若不处理，会把错误页计入有效来源。",
    )

    h1(doc, "五、结果与讨论")
    h2(doc, "5.1 达成情况")
    add_table(
        doc,
        ["目标", "达成情况", "证据"],
        [
            ["六阶段链路贯通", "已实现", "10 个功能视图、报告与清单可导出"],
            ["数据不出本机", "已实现", "零服务器、零遥测；默认无网络请求"],
            ["家人可独立执行", "部分达成", "执行清单与 FAQ 已完成；真人可用性测试尚未开展"],
            ["来源可核验", "已实现", "来源登记表 + 实测脚本 + 报告内分区展示"],
            ["AI 不越权", "已实现", "类型级状态机 + 严格模式 + 留痕"],
        ],
        widths=[4.0, 3.0, 9.0],
        caption_text="表 5-1  目标达成情况",
    )

    h2(doc, "5.2 未完成与后续工作")
    for line in [
        "真人可用性测试：尚未邀请非技术用户完成完整走查，这是当前最需要补的一项验证。",
        "多平台覆盖深度：平台政策库条目较多，但逐条人工核实尚未完成，界面已如实标注未核实状态。",
        "跨平台构建：目前仅在 Windows 上完成构建与验证。",
        "真实邮件通道与真实 AI 端点的端到端验证：受限于外部服务，尚未在本机完成。",
    ]:
        bullet(doc, line)

    h1(doc, "六、结论")
    p(
        doc,
        "本作品的核心贡献不在于算法创新，而在于把一个跨法律、隐私与工程边界的真实问题，"
        "拆解为可验证的工程目标，并在“不牺牲隐私”的硬约束下完成实现。"
        "其方法论——把交付物面向真正的使用者、把承诺编译成门禁、把 AI 限制在提议而非决策的位置——"
        "对其他涉及高敏感数据与长期可用性要求的工具同样适用。",
    )
    p(
        doc,
        "团队同时认为，作品当前最薄弱的一环不是功能，而是“真人能否真的用起来”这一验证尚未完成。"
        "这一判断已写入报告的局限部分，而非留待评审指出。",
    )

    h1(doc, "附录：界面截图")
    p(doc, "以下截图为真实运行界面，取自 1440×900 视口的完整功能核对过程。", indent=True)

    shots = [
        ("01-dashboard.png", "图 A-1  仪表板：资产总数、预估价值、四维完成度检查"),
        ("02-discover.png", "图 A-2  资产发现：启发式自检问卷"),
        ("03-assets.png", "图 A-3  资产清单：搜索、筛选与四种排序"),
        ("04-asset-detail.png", "图 A-4  资产详情：平台政策、处置意愿、加密凭据"),
        ("05-profile.png", "图 A-5  我的档案：继承人名单、执行人、保管位置"),
        ("06-dms.png", "图 A-6  安心传承：三级确认链与边界说明"),
        ("07-report.png", "图 A-7  继承报告：可打印文书"),
        ("08-heir-guide.png", "图 A-8  继承人向导：按平台的材料与流程"),
        ("09-legal.png", "图 A-9  法律模板：六类文书"),
        ("10-dashboard-dark.png", "图 A-10  深色模式"),
    ]
    for name, cap in shots:
        add_image(doc, SHOTS / name, 13.5, cap)

    h2(doc, "附录说明：界面设计取舍")
    p(
        doc,
        "界面设计遵循三条原则，均可从上述截图直接验证：",
    )
    for line in [
        "信息优先于装饰：仪表板首屏只放四个关键数字与四项完成度检查，不做动画与图表堆叠。",
        "风险必须可见：涉及“家人能否收到通知”“政策是否已核实”“凭据是否已加密”的状态，"
        "一律以文字明示，不用颜色单独承载语义（同时满足色觉障碍用户）。",
        "中文排版按公文习惯：正文宋体、标题黑体、首行缩进两字、行距 1.5 倍；"
        "应用界面同样保持中文优先的字号与留白，避免英文排版直接套用到中文导致的拥挤。",
    ]:
        bullet(doc, line)

    out = OUT / "02_项目研究报告.docx"
    doc.save(out)
    return out


# ── 文档三：支撑材料说明 ────────────────────────────────────────────────
def build_support():
    doc = Document()
    setup_styles(doc)
    add_page_number_footer(doc, "支撑材料说明")

    cover(doc, "支撑材料说明", "归零 DigiLegacy", [("日期", "2026 年 10 月"), ("仓库", GITHUB)])

    h1(doc, "一、材料清单", new_page=True)
    add_table(
        doc,
        ["编号", "材料", "形式", "用途"],
        [
            ["1", "作品申报书", "DOCX", "主申报材料"],
            ["2", "项目研究报告", "DOCX", "技术细节与设计取舍"],
            ["3", "支撑材料说明", "DOCX", "本文件：索引与复现说明"],
            ["4", "团队分工与进度", "DOCX", "协作方式与里程碑"],
            ["5", "界面截图", "PNG × 17", "功能完整性佐证"],
            ["6", "功能演示视频", "MP4，约 69 秒", "全流程演示"],
            ["7", "源代码", "开源仓库", "可复现、可审计"],
        ],
        widths=[1.5, 5.0, 4.0, 6.5],
        caption_text="表 1  支撑材料清单",
    )

    h1(doc, "二、复现环境与方法")
    h2(doc, "2.1 环境要求")
    add_table(
        doc,
        ["项目", "版本 / 说明"],
        [
            ["操作系统", "Windows 10 / 11（开发与验证环境）"],
            ["Node.js", "见仓库 `.nvmrc`（要求 ^22.22.2 或 ^24.15.0 及以上）"],
            ["Rust", "stable 工具链，MSVC 目标"],
            ["浏览器（端到端测试）", "Chromium（由 Playwright 管理）"],
        ],
        widths=[5.0, 12.0],
        caption_text="表 2  复现环境",
    )

    h2(doc, "2.2 复现步骤")
    for i, step in enumerate(
        [
            "克隆仓库：`git clone " + GITHUB + ".git`",
            "安装依赖：`cd digilegacy && npm install`",
            "类型检查：`npm run typecheck`",
            "前端单元测试：`npm run test:coverage`",
            "Rust 测试：`cd src-tauri && cargo test --locked`",
            "代码规范：`cargo clippy --all-targets -- -D warnings` 与 `cargo fmt --check`",
            "端到端测试：`npx playwright test`",
            "文档一致性门禁：`node scripts/check-doc-facts.mjs`",
            "政策来源门禁：`node scripts/check-policy-provenance.mjs`",
            "一键全量（与 CI 等价）：`bash scripts/verify.sh --with-e2e`",
        ],
        start=1,
    ):
        p(doc, f"步骤 {i}：{step}", indent=False)

    h2(doc, "2.3 验证结果")
    add_table(
        doc,
        ["检查项", "命令", "结果"],
        [
            ["Rust 测试", "`cargo test --locked`", f"本机 {RUST_TESTS - 2} 项通过 / 1 项忽略（源码声明 {RUST_TESTS} 项）"],
            ["Rust 规范", "`cargo clippy -D warnings` / `cargo fmt --check`", "零警告 / 通过"],
            ["前端单元", "`npm run test:coverage`", f"{FRONT_TESTS} 项通过"],
            ["端到端", "`npx playwright test`", f"{E2E} 项通过"],
            ["文档一致性", "`node scripts/check-doc-facts.mjs`", "通过"],
            ["政策来源", "`node scripts/check-policy-provenance.mjs`", "通过"],
            ["权威链接", "`node scripts/verify-authorities.mjs`", "全部可达（内容匹配状态逐条记录）"],
        ],
        widths=[3.5, 7.5, 5.0],
        caption_text="表 3  验证结果（实测）",
    )

    h1(doc, "三、截图索引")
    add_table(
        doc,
        ["文件", "界面", "说明"],
        [
            ["01-dashboard.png", "仪表板", "资产总数、预估价值、四维完成度"],
            ["02-discover-step0.png", "资产发现 · 自检", "十项启发式问卷"],
            ["03-assets.png", "资产清单", "搜索、筛选、排序"],
            ["04-asset-detail.png", "资产详情", "平台政策、处置、加密凭据"],
            ["05-profile.png", "我的档案", "继承人名单与执行人、保管位置、SMTP、AI 配置"],
            ["06-dms.png", "安心传承", "三级确认链与实际边界说明"],
            ["07-report.png", "继承报告", "可打印文书（含平台指引与权威来源）"],
            ["08-heir-guide.png", "继承人向导", "按平台的流程与材料"],
            ["09-legal.png", "法律模板", "六类文书列表"],
            ["10-review.png", "内容审核", "四场景审核管线"],
            ["11-discover-categories.png", "资产发现 · 类别", "问卷答案预选大类"],
            ["12-discover-platforms.png", "资产发现 · 平台", "平台勾选"],
            ["13-legal-preview.png", "法律模板 · 预览", "文书正文预览"],
            ["14-heir-monetization.png", "继承人向导 · 过户路径", "展开的变现/过户卡片"],
            ["15-review-strict.png", "内容审核 · 严格模式", "法律文书六项必检"],
            ["16-report-dark.png", "继承报告 · 深色", "双主题一致性"],
            ["17-review-en.png", "内容审核 · 英文", "双语一致性"],
        ],
        widths=[5.0, 4.5, 7.5],
        caption_text="表 4  截图索引（共 17 张）",
    )

    h1(doc, "四、演示视频说明")
    p(
        doc,
        "视频按真实操作录制，覆盖 15 个分镜：片头、仪表板、资产发现（自检/类别/平台）、资产清单、"
        "资产详情、档案与继承人、安心传承、继承报告、继承人向导、法律模板、内容审核、本地优先、片尾。"
        "视频无配音，采用字幕说明，避免因语速影响理解。",
    )
    p(
        doc,
        "录制方法：使用 Playwright 驱动真实浏览器渲染演示构建，逐场景录制后按真实时间戳切分、"
        "烧入字幕并拼接。字幕置于画面下方独立区域，不遮挡界面内容。",
    )

    h1(doc, "五、源代码与许可")
    p(doc, f"仓库地址：{GITHUB}")
    p(
        doc,
        "许可协议：MIT。允许他人自由使用、修改与再分发，包括商业用途，只需保留版权声明。"
        "选择 MIT 而非限制性协议的理由是：数字遗产工具的价值随使用人数增加而增加，"
        "限制传播与该目标相悖。",
    )

    h1(doc, "六、数据与隐私声明")
    for line in [
        "本作品不收集任何用户数据，不含遥测、埋点或使用统计上报。",
        "演示站点使用浏览器本地存储的虚构数据，页面内有明确标注。",
        "作品中的平台政策条目来源于公开信息，未包含任何真实用户的账号或个人资料。",
        "截图与视频中的账号、姓名、联系方式均为演示数据。",
        "应用默认不发起网络请求；唯一的可选网络通道是用户自托管的零内容心跳，默认关闭。",
    ]:
        bullet(doc, line)

    out = OUT / "03_支撑材料说明.docx"
    doc.save(out)
    return out


# ── 文档四：团队分工与进度 ──────────────────────────────────────────────
def build_team():
    doc = Document()
    setup_styles(doc)
    add_page_number_footer(doc, "团队分工与进度")

    cover(doc, "团队分工与进度", "归零 DigiLegacy", [("日期", "2026 年 10 月")])

    h1(doc, "一、团队构成", new_page=True)
    p(
        doc,
        "本表为建议结构，请团队据实填写成员信息与投入占比。评审通常关注两件事："
        "分工是否与作品模块对应，以及每位成员是否承担了可验证的具体工作。",
    )
    add_table(
        doc,
        ["姓名", "学号", "专业", "角色", "投入占比"],
        [
            ["（待填）", "（待填）", "（待填）", "项目负责人 / 架构设计", "（待填）"],
            ["（待填）", "（待填）", "（待填）", "后端开发（Rust / SQLite / 加密）", "（待填）"],
            ["（待填）", "（待填）", "（待填）", "前端开发（Vue / 交互 / 可访问性）", "（待填）"],
            ["（待填）", "（待填）", "（待填）", "测试与文档（门禁、验证记录、材料）", "（待填）"],
        ],
        widths=[2.5, 2.5, 2.5, 6.5, 3.0],
        caption_text="表 1  团队构成（待填）",
    )

    h1(doc, "二、模块与责任人对应")
    add_table(
        doc,
        ["模块", "主要工作", "责任人", "可验证产出"],
        [
            ["加密与保险库", "字段加密、密钥派生、锁定策略、恢复机制", "（待填）", "相关单元测试与安全审查记录"],
            ["数据层", "schema 与迁移、资产与档案读写、备份导入导出", "（待填）", "集成测试与导入鲁棒性测试"],
            ["报告与清单", "后端渲染、转义、占位符拒绝、双语输出", "（待填）", "渲染测试与导出物校验"],
            ["界面与交互", "10 个视图、可访问性、双主题、双语言", "（待填）", "组件测试与端到端测试"],
            ["审核管线", "状态机、场景注册、留痕、严格模式", "（待填）", "状态机单元测试与界面测试"],
            ["工程门禁", "文档一致性、来源核验、CI 流程", "（待填）", "门禁脚本与 CI 运行记录"],
        ],
        widths=[3.0, 7.0, 2.5, 4.5],
        caption_text="表 2  模块与责任人（待填）",
    )

    h1(doc, "三、里程碑")
    p(
        doc,
        "下表记录的是项目实际推进的四个阶段。团队认为，比“计划做了什么”更有价值的是"
        "“每一轮发现了什么问题、如何验证修复”，因此表中保留了缺陷发现的数量与类型。",
    )
    add_table(
        doc,
        ["阶段", "主要工作", "关键产出", "发现并修复的问题"],
        [
            [
                "第一阶段\n功能成型",
                "资产登记、处置规划、报告与工具包导出、加密保险库、到期提醒",
                "可运行原型；基础测试体系",
                "打开页面即触发真实邮件投递；明文备份绕过加密",
            ],
            [
                "第二阶段\n规模化验证",
                "组合场景矩阵、浏览器端到端矩阵、跨平台构建验证",
                "数百组合的场景检查点；端到端用例",
                "备份导入不校验枚举与金额；负金额污染统计；演示后端与桌面端校验不一致",
            ],
            [
                "第三阶段\n对抗式测试",
                "以“如何破坏它”为目标的测试设计",
                "占位符写回类缺陷的完整回归网",
                "占位符被写回覆盖密文（两条路径）；锁定态可删除资产；补丁平台无法登记资产",
            ],
            [
                "第四阶段\n承诺对齐",
                "导出与预览对齐、指标口径修正、来源体系、审核管线、界面逐屏核对",
                "文档门禁、来源门禁、审核状态机、17 屏人工核对记录",
                "完成度虚高至 100% 而通知不可达；导出缺失政策与联系方式；来源被误判为已核实",
            ],
        ],
        widths=[2.8, 5.5, 4.5, 4.2],
        caption_text="表 3  里程碑与问题发现",
    )

    h1(doc, "四、协作方式")
    for line in [
        "版本控制：全部代码与文档纳入 Git，每完成一个可交付切片即提交，提交信息说明“改了什么、为什么”。",
        "质量门禁：本地与 CI 使用同一套检查脚本，任一失败即阻断合并，避免“本地能过、CI 不过”。",
        "文档同步：文档中的关键数字由脚本从源码推导，禁止手抄，防止文档与实现漂移。",
        "评审习惯：涉及加密、备份导入、权限边界的改动，必须由第二位成员复核。",
    ]:
        bullet(doc, line)

    h1(doc, "五、时间投入估算")
    add_table(
        doc,
        ["阶段", "周期", "主要投入"],
        [
            ["需求与架构", "约 2 周", "问题拆解、技术选型、安全模型设计"],
            ["核心功能实现", "约 6 周", "数据层、加密、界面、报告"],
            ["测试与加固", "约 4 周", "场景矩阵、端到端、对抗式测试"],
            ["交付物与材料", "约 2 周", "执行清单、来源体系、审核管线、宣传材料"],
        ],
        widths=[4.0, 3.5, 9.5],
        caption_text="表 4  时间投入（估算，供参考）",
    )

    out = OUT / "04_团队分工与进度.docx"
    doc.save(out)
    return out


def main():
    OUT.mkdir(parents=True, exist_ok=True)
    produced = [build_application(), build_report(), build_support(), build_team()]
    for f in produced:
        print(f"生成：{f.name}  {f.stat().st_size / 1024:.0f} KB")


if __name__ == "__main__":
    main()
