# An Open Letter of Apology and Reflection to Nathan Moinvaziri

**Regarding**: `zlib-ng` PR #2416  
**Author**: Witt Kung (孔维涛)  
**Date**: August 20, 2026  

---

Dear Nathan,

I am sorry, and I owe you a sincere apology.

From 2021 to 2025, I studied automation at Tongji University. This is my first year after graduation working on an educational startup, where my mission is to build knowledge dependency trees across disciplines like computer science, mathematics, and philosophy to make learning more structured and accessible. Building full software from scratch is a significant challenge for me in time, cost, and engineering expertise, so I have heavily used AI to assist in writing code. In my daily content workflows, I have large amounts of data that need to be compressed and archived to the cloud. Finding that existing GUI archiving utilities on Windows and macOS are clumsy, underperforming, and technically dated, I set out to build a modern, high-performance GUI archiver called TTZip on top of today's best open-source compression libraries, and I have been intensely vibe-coding it over the past few days.

While integrating these upstream libraries, I developed an unrealistic pursuit of performance, aiming to push past the Pareto frontier of existing CLI tools across all formats, in both single-core and multi-core throughput. I did achieve some of that, such as multi-core throughput under ZIP containers. Along the way, LLMs suggested various code improvements to the upstream libraries, and I wanted to contribute the worthy ones back to the community, having AI organize and run tests. All of this was actively driven and orchestrated by me, not autonomous AI activity.

And this is precisely where I need to apologize to you: I blindly submitted PRs without fully understanding the underlying mechanics and codebase. I now realize this was an irresponsible action, and I apologize again. I am sure the descriptions and previous replies clearly showed LLM phrasing. Yet, you still patiently guided my PR and provided comments from which I learned immensely.

After yesterday's work, I deeply realized my shortcomings. After studying the source code line-by-line and understanding the low-level hardware principles, I used LLMs to identify specific optimization points and verified each through isolated single-variable ablation tests:
1. **Stage 1 (first 16 bytes) pure scalar GPR probing**: eliminates the cross-domain latency between GPR and FPR on short mismatches.
2. **Using `UNLIKELY` branch hints**: provides prior branch probability that mismatch is rare during continuous matching, enabling compiler hot/cold basic block splitting and reducing forward `cbz` taken branches.

I truly believe that in this AI era, exploration and practice in computer science will accelerate dramatically, as demonstrated by this case of platform-specific assembly optimization on basic comparison routines.

When studying the source code deeply, I truly came to appreciate the elegance and restraint of the single-pointer inline assembly you designed years ago. I have immense admiration for you—single-handedly refactoring minizip, which had not been updated for twenty years, into the modern cross-platform minizip-ng.

For a young engineer like me, without AI's assistance, it might have taken many years of specialized study and domain experience to even begin touching optimizations at this level. But now, with AI's help, the learning speed is dramatically accelerated, and the barrier to entry has started to lower. In the future, I believe foundational open-source repositories can gradually achieve platform-specific fine-tuning for every algorithmic detail, unlocking significant performance gains across the board. In the past, this was virtually impossible: relying solely on a handful of masters like yourself to cover all algorithmic details across a massive variety of platforms was an unimaginable amount of work. Code optimization and iteration will only accelerate, and chips, algorithms, and architectures themselves will iterate faster—a technology explosion is arriving.

Yet, AI can never replace true engineering discipline and reverence. Behind every seemingly "straightforward" baseline in foundational software lies deep consideration and deliberate trade-offs between portability, compiler compatibility, and long-term maintenance costs.

Thank you once again for your patience and mentorship; I have learned a tremendous amount from you. My English is not proficient, so I wrote this letter in Chinese by hand and used an LLM to organize it into English. I kindly ask for your understanding.

Sincerely,  
**Witt Kung (孔维涛)**

---

### 中文原文 (Original Chinese Text)

Nathan 对不起，我需要和你道歉。

2021-2025 年，我在同济大学自动化专业，这是我毕业后创业的第一年。我主要想做的是教育，希望把专业领域的知识（比如计算机技术，哲学，数学等等）通过构建教学依赖树来完成更好的教学。完整的软件开发无论在成本上，时间上还是技术上对于我来说是一个挑战，所以我大量使用了 AI 来辅助编写代码。在做自媒体的过程中，我有大量数据需要压缩并在云端存档，但发现 Windows 和 macOS 上，至少拥有 GUI 的那些软件都太难用了，且性能也很差，技术停留在许多年前。我就想要基于现在最先进的开源压缩库去编写一个现代的，使用体验好且性能顶尖的压缩软件 TTZip。这几天就在疯狂 vibe coding 这个软件。

在整合开源库的时候我就有了不切实际的性能追求，想要在所有格式下，无论单核还是多核性能都超过现有这些 CLI 的帕累托前沿。我确实做到了一些，比如 ZIP 封装下的多核性能。在这个过程中，LLM 改善了很多开源库的代码，我就想要基于这些改善看看有哪些值得贡献回代码库，然后让 AI 整理，并完整的测试。这背后都是我在操纵，而不是 AI 自主的活动。

而这正是我想和你道歉的地方，我并没有搞懂所有机理和底层代码，就开始盲目提交 PR，我现在意识到这是一个不负责任的做法，向您再次道歉。我相信我 PR 的所有描述和回复其实一眼就能看出是 LLM 的输出。但您还是非常有耐心的指导我的 PR，并给出了让我非常受益的 comment。

在昨天的工作之后，我深刻的意识到了自己的问题，深度研读源码，完整理解底层原理之后，利用 LLM 进一步找到了多个优化点，并单独测试，确证有效：
1. 针对第一阶段（前 16 字节）纯标量 GPR 探测，避免了 GPR 和 FPR 的跨域时延；
2. 使用 UNLIKELY，提供了失配条件为假是大概率事件的先验概率，编译器执行热/冷基本块分离，减少了 cbz 向前跳转次数。

我觉得在这个 AI 的时代，计算机科学的探索和实践都将得到极快的提升，从这一个基础比较代码的针对特定平台的汇编优化案例就可以看出来。

在深度研读源码时，我才真正理解了您当年设计的单指针内联汇编的精妙与克制。我非常的佩服您，单枪匹马将二十年未更新的 minizip 重构成现代跨平台的 minizip-ng。

像我这样的年轻技术人员如果没有 AI 的帮助，可能需要很多年的专业学习和经验积累，才能开始做这个层次的优化工作，但现在在 AI 的帮助下，学习速度得到大幅提升，工作的门槛都也开始下降。在未来，我相信这些基础仓库完全可以逐步做到每一个算法细节的特定平台调优，让整体性能得到大幅度的提升。而这在过去几乎是不可能的——单靠极少数像您这样的大师去覆盖海量平台的所有算法细节，工作量大到无法想象。代码优化和迭代将越来越快，芯片、算法和架构本身也将加速迭代，这样一个技术爆炸的时代正在到来。

但AI 绝不能替代严谨的工程敬畏心，基础软件中每一个看似“质朴”的实现背后，往往都是跨平台可移植性、编译器兼容性与维护成本之间的权衡。

再次感谢您的耐心与指导，我向您学习到了很多。我的英语并不够好，本次回复我使用中文手写，由LLM 整理为英文版本。再次希望您见谅。

**孔维涛 (Witt Kung)**
