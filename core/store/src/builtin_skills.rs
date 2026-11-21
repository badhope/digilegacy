//! Built-in skill pack shipped with schema v18.
//!
//! These are reviewable prompt fragments, not executable code. They are inserted
//! once when the v18 migration runs; users may edit or delete them afterwards and
//! they will not be resurrected on the next launch.

use chrono::Utc;
use rusqlite::{params, Connection};

pub struct BuiltinSkill {
    pub name: &'static str,
    pub description: &'static str,
    pub instructions: &'static str,
    pub enabled: bool,
}

/// The starter pack is deliberately small and operational. Three core skills are
/// enabled so a fresh install can plan an inventory/executor handoff immediately;
/// the rest stay visible but disabled to keep the planner prompt bounded.
pub const BUILTIN_SKILLS: &[BuiltinSkill] = &[
    BuiltinSkill {
        name: "数字资产盘点",
        description: "把分散的账户整理成可复核的数字遗产清单，标出高价值、高敏感与待核实项。",
        enabled: true,
        instructions: r#"目标：把用户口述或平台已有的数字账户整理成可执行、可复核的资产清单。不要编造账户、余额、联系人或平台政策。

工作顺序：
1. 先调用 assets.discover 和 assets.list 读取已有资产，标出重复项与缺口；不要重复创建同一条资产。
2. 按类别补齐：邮箱与身份、社交与内容、金融与支付、云存储与照片、订阅与会员、域名与网站、工作与协作、游戏与虚拟物品、加密资产、其他。
3. 每个账户至少记录：平台、账号标识、用途、是否有金钱或情感价值、身后处置意愿（保留/纪念/转移/删除/待定）、执行人是否需要访问、最后核对日期。
4. 一次只问一组缺失字段，优先问高价值与高敏感项；用户不回答时保留“待核实”，不要替他做决定。
5. 敏感凭据只记录“存放位置/持有人/获取条件”，绝不在清单、摘要或聊天里回显密码、助记词、恢复码。

输出顺序：资产表（Markdown 表格）→ 缺口清单 → 下一步三项动作。结尾注明哪些条目未经用户确认、哪些需要律师或平台客服核实。"#,
    },
    BuiltinSkill {
        name: "执行人交付手册",
        description: "给执行人生成按阶段推进的操作手册，不依赖内嵌明文密码。",
        enabled: true,
        instructions: r#"目标：为执行人（或受托人）生成一份可以直接照着做的交付手册，而不是泛泛的“联系平台”。

工作顺序：
1. 读取 assets.list、policies.list 与已有 deliver.export 内容，先确认哪些账户、政策和交付物已经存在。
2. 按阶段组织：确认法律文件与死亡证明 → 通知核心平台 → 提交平台 Legacy/纪念/删除请求 → 取得密码库应急访问 → 处理加密资产与硬件钱包 → 备份照片与文档 → 取消订阅与自动扣费 → 关闭或 memorialize 社交账号。
3. 每一步写清：触发条件、需要的材料、平台入口、预计等待时间、失败后的升级路径、要记录的字段（工单号、日期、经办人）。
4. 不承诺平台一定批准；把需要公证、法院、律师或跨境程序的环节单独标出。
5. 生成可打印清单；密码、助记词、恢复码只写“存放在哪里、由谁保管”，不写明文。

输出：阶段表 → 每阶段 checklist → 风险与升级路径 → 需要用户现在补充的三项材料。"#,
    },
    BuiltinSkill {
        name: "平台遗产政策核对",
        description: "核对各平台的 Legacy、纪念化与删除工具，区分已核实与待核实。",
        enabled: true,
        instructions: r#"目标：核对平台是否提供身后账户工具，并把“官方已核实”与“待核实”分开。

工作顺序：
1. 先调用 policies.list / policies.get 读取平台内置政策快照，记录政策版本与快照日期。
2. 需要外部信息时，只引用可公开访问的来源，写明 URL、发布日期和适用地区；不要假装登录平台或声称已完成操作。
3. 输出表格：平台、功能名称、可指定对象、可访问的数据范围、生效条件、等待时限、是否优先于遗嘱、来源 URL、核实日期。
4. 对没有官方工具的账户，给出替代路径：执行人联系客服、数据导出、法律程序、密码库应急访问。
5. 提醒用户平台条款可能优先于遗嘱，最终应由当地律师复核；不要把通用做法说成法律结论。

如果来源打不开或条款有地区差异，明确写“待核实”，并给出下一次核对动作。"#,
    },
    BuiltinSkill {
        name: "凭据与恢复材料交接",
        description: "设计密码库、恢复码与加密资产材料的交接方案，避免明文进入遗嘱。",
        enabled: false,
        instructions: r#"目标：设计凭据与恢复材料的交接方案，不让密码、助记词或恢复码进入遗嘱正文或公开文档。

工作顺序：
1. 分三类盘点：密码库主口令与应急访问、平台恢复码/备份码、加密钱包助记词与硬件钱包位置。
2. 建议至少两人知晓材料位置；使用密码管理器的 emergency access 或密封分片存放；不要把恢复短语拍照或同步到云笔记。
3. 输出：材料清单、存放位置、持有人、获取条件、上次演练日期、下次复核日期。
4. 在对话中只讨论“位置与流程”，不要求用户粘贴明文；如果用户主动粘贴，提醒其立即轮换并停止回显。
5. 说明风险：单点丢失、继承人互相不信任、硬件钱包损坏、跨境访问限制。"#,
    },
    BuiltinSkill {
        name: "纪念与删除意愿",
        description: "按账户梳理保留、纪念化、转移、删除四种身后处置意愿及其后果。",
        enabled: false,
        instructions: r#"目标：帮用户逐账户表达身后处置意愿，不替用户做价值判断。

对每个账户给出四选一：保留、纪念化、转移给指定人、删除；无法决定时标“待定”。说明每种选择的实际后果：
- 保留：账户可能长期无人维护，仍可能扣费或被回收。
- 纪念化：资料可见范围、谁能管理、是否还能登录，因平台而异。
- 转移：需要平台是否允许转让、是否涉及实名或版权。
- 删除：是否可逆、多久生效、是否影响关联数据。

同时覆盖：云照片与家庭共享、域名与网站、付费内容与版权、游戏账号与虚拟物品。输出按平台分组的意愿表，并列出需要本人在世时提前设置的平台工具。"#,
    },
    BuiltinSkill {
        name: "年度复核与演练",
        description: "每年复核资产、平台政策、执行人权限与备份可读性，输出差异和待办。",
        enabled: false,
        instructions: r#"目标：把数字遗产计划变成每年可执行的复核，而不是写完就放着。

复核清单：
1. 资产增减：新增账户、关闭账户、余额或价值变化。
2. 平台政策：Legacy 工具、删除政策、费用与等待期是否变化。
3. 人员：执行人、备用联系人、紧急联系人是否仍可用。
4. 凭据：密码库应急访问是否仍有效；恢复码是否过期；做一次不暴露明文的演练。
5. 加密资产：硬件钱包、助记词存放位置、交易所账户状态。
6. 订阅：仍在扣费的会员、域名、云存储。
7. 备份：照片和文档备份是否可读、是否加密、执行人能否取得。
8. 法律文件：遗嘱/信托/授权书是否仍与现状一致。

输出：本次变化 → 失效项 → 未来 30 天待办 → 下次复核日期。不要只给“建议定期复核”这种空话。"#,
    },
    BuiltinSkill {
        name: "家属通知草稿",
        description: "生成克制、可执行的家属/执行人通知草稿，不群发敏感凭据。",
        enabled: false,
        instructions: r#"目标：生成一份给家属或执行人的通知草稿，重点是“先做什么”，不是情绪化的长信。

草稿结构：
1. 一句确认事实与身份，不猜测死因或法律责任。
2. 需要准备的材料：死亡证明、关系证明、遗嘱/授权书、平台工单所需信息。
3. 最先做的三件事：联系谁、提交什么、记录什么。
4. 明确不要做的事：不要公开密码、不要删除账户、不要急于转移加密资产、不要向未核实身份的人透露资料。
5. 联系人清单与等待时间；留出对方确认与补充的空间。
6. 按用户所在地、语言和文化习惯调整语气；不要代替法律或医疗通知。

输出中文草稿，并附一份更短的短信/即时通讯版本。敏感信息只写“另行安全交付”。"#,
    },
    BuiltinSkill {
        name: "法律与管辖边界",
        description: "识别数字遗产问题中的法律边界与跨境差异，提示何时必须找律师。",
        enabled: false,
        instructions: r#"目标：给数字遗产任务加一道法律边界，不把平台操作建议包装成法律意见。

工作方式：
1. 先确认用户所在地、账户所在平台的主要管辖地、是否有跨境资产；未知时明确假设。
2. 区分四类规则：继承法/遗嘱形式、平台服务条款、隐私与数据保护法、加密资产监管。
3. 美国场景可提示 RUFADAA 及各州差异；中国场景提示个人信息保护、继承与公证程序；其他地区只给“需当地律师核实”的框架，不编造条文。
4. 输出：已确认事实、法律不确定点、必须找律师/公证/法院的事项、可先做的非法律动作。
5. 不提供规避平台审核、绕过继承人认证或隐藏资产的建议。

结尾固定声明：这是一般信息整理，不构成法律意见；涉及签字、公证、跨境或大额资产时必须由持牌专业人士复核。"#,
    },
];

/// Insert the pack exactly once. `name` is unique, so an existing user-created
/// skill with the same name wins and is never overwritten.
pub fn seed(conn: &Connection) -> rusqlite::Result<()> {
    let now = Utc::now().to_rfc3339();
    for skill in BUILTIN_SKILLS {
        conn.execute(
            "INSERT OR IGNORE INTO agent_skills
             (name, description, instructions, enabled, builtin, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, 1, ?5, ?5)",
            params![
                skill.name,
                skill.description,
                skill.instructions,
                if skill.enabled { 1 } else { 0 },
                now
            ],
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pack_is_bounded_and_has_three_core_skills() {
        assert_eq!(BUILTIN_SKILLS.len(), 8);
        assert_eq!(
            BUILTIN_SKILLS.iter().filter(|skill| skill.enabled).count(),
            3
        );
        let enabled_chars: usize = BUILTIN_SKILLS
            .iter()
            .filter(|skill| skill.enabled)
            .map(|skill| skill.instructions.chars().count())
            .sum();
        assert!(
            enabled_chars < 8_000,
            "enabled built-in skills must stay well below the 12k planner budget"
        );
    }
}
