use dialoguer::{theme::ColorfulTheme, Input};
use twofloat::TwoFloat as F128;

fn main() {
    let n: usize = Input::with_theme(&ColorfulTheme::default())
        .with_prompt("Erasure coding parameter n")
        .with_initial_text("8")
        .interact()
        .unwrap();

    let k: usize = Input::with_theme(&ColorfulTheme::default())
        .with_prompt("Erasure coding parameter k")
        .with_initial_text("4")
        .interact()
        .unwrap();

    eprintln!("(n,k) = ({n},{k})");

    let p_sla: usize = Input::with_theme(&ColorfulTheme::default())
        .with_prompt("SLA Level (x nines)")
        .with_initial_text("11")
        .interact()
        .unwrap();
    let p_sla = 1.0 - F128::powi(F128::from(0.1f64), p_sla as i32);

    eprintln!("SLA Level = {} (%)", f64::from(p_sla * 100.));

    let comb = comb_table(n);

    let f = |p: F128| {
        let mut sum = F128::from(0.0f64);
        for i in 0..=(n - k) {
            sum += F128::from(comb[n][i] as f64) * pow(p, i as i32) * pow(1. - p, (n - i) as i32);
        }
        sum
    };

    // Validate the function is monotonically decreasing in [0,1]
    for x in 0..(1000000 - 1) {
        let base = F128::from(0.000001f64);
        let p1 = base * x as f64;
        let p2 = base * (x + 1) as f64;
        assert!(f(p1).hi() >= f(p2).hi());
    }

    let mut l = F128::from(0.0f64);
    let mut r = F128::from(1.0f64);

    while r.hi() - l.hi() > 0. {
        let mid = (l + r) / 2.;
        let y = f(mid);
        if y < p_sla {
            r = mid;
        } else {
            l = mid;
        }
    }

    let p_found = l;
    eprintln!("p (solved) = {}", f64::from(p_found));

    let mtbf: f64 = Input::with_theme(&ColorfulTheme::default())
        .with_prompt("MTBF (H)")
        .with_initial_text("1000000")
        .interact()
        .unwrap();

    let recovery_time = p_found * mtbf;
    eprintln!("Recovery interval <= {} (H)", f64::from(recovery_time));
}
fn pow(x: F128, k: i32) -> F128 {
    if k == 0 {
        F128::from(1.0f64)
    } else {
        F128::powi(x, k)
    }
}
fn comb_table(n_max: usize) -> Vec<Vec<i64>> {
    let mut dp = vec![vec![0; n_max + 1]; n_max + 1];
    for i in 0..=n_max {
        for j in 0..i + 1 {
            if j == 0 || j == i {
                dp[i][j] = 1;
            } else {
                dp[i][j] = dp[i - 1][j - 1] + dp[i - 1][j];
            }
        }
    }
    dp
}
