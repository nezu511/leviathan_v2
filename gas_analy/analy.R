library(ggplot2)
library(dplyr)

# 1. データの読み込み
df <- read.csv("stRevertTest_benchmarks.csv", header=TRUE)

# 2. 前処理
df_clean <- df %>%
  filter(Status == "Success") %>%
  filter(Time_us > 0) %>%
  mutate(Address = as.factor(Address))

# 3. 統計サマリーの表示
# アドレスごとに、1ガスあたりの平均実行時間を計算
summary_stats <- df_clean %>%
  group_by(Address) %>%
  summarize(
    Count = n(),
    Avg_Time_us = mean(Time_us),
    Avg_Gas = mean(Gas),
    Us_per_Gas = mean(Time_us / Gas)
  )

print("--- アドレス別統計サマリー ---")
print(summary_stats)
