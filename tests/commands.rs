use assert_cmd::Command;
use std::{collections::HashMap,
          thread,
          time::{Duration, SystemTime, UNIX_EPOCH}};

/// Return current unix timestamp + offset, waiting until we're close to the start of a whole
/// second to make tests more reproducible.
fn ts(secs: i64) -> i64 {
    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
    if now.subsec_millis() > 100 {
        thread::sleep(Duration::from_millis(25));
        ts(secs)
    } else {
        now.as_secs() as i64 + secs
    }
}

/// Return a `Command` for the main binary (compiled by cargo) with the given args.
/// For convenience, "%F" is replace by "-F tests/emerge.".
fn emlop(args: &str) -> Command {
    let mut e = Command::new(env!("CARGO_BIN_EXE_emlop"));
    e.env("TZ", "UTC");
    e.env("EMLOP_CONFIG", "");
    e.args(args.replace("%F", "-F tests/emerge.").split_whitespace());
    e
}

/// Run emlop, check for success, return output as string
fn emlop_out(args: &str) -> String {
    let out = emlop(args).output().expect(&format!("could not run emlop {:?}", args));
    assert!(out.status.success());
    assert!(out.stderr.is_empty());
    String::from_utf8(out.stdout).expect("Invalid utf8")
}

#[test]
fn log() {
    let t = [// Basic test
             ("%F10000.log l client -oc",
              "2018-02-04 04:55:19    35:46 >>> mail-client/thunderbird-52.6.0\n\
               2018-02-04 05:42:48    47:29 >>> www-client/firefox-58.0.1\n\
               2018-02-09 11:04:59    47:58 >>> mail-client/thunderbird-52.6.0-r1\n\
               2018-02-12 10:14:11       31 >>> kde-frameworks/kxmlrpcclient-5.43.0\n\
               2018-02-16 04:41:39  6:03:14 >>> www-client/chromium-64.0.3282.140\n\
               2018-02-19 17:35:41  7:56:03 >>> www-client/chromium-64.0.3282.167\n\
               2018-02-22 13:32:53       44 >>> www-client/links-2.14-r1\n\
               2018-02-28 09:14:37     6:02 >>> www-client/falkon-3.0.0\n\
               2018-03-06 04:19:52  7:42:07 >>> www-client/chromium-64.0.3282.186\n\
               2018-03-12 10:35:22       14 >>> x11-apps/xlsclients-1.1.4\n\
               2018-03-12 11:03:53       16 >>> kde-frameworks/kxmlrpcclient-5.44.0\n\
               2018-03-14 01:36:42       10 >>> www-client/falkon-24.08.3\n"),
             // Check output when duration isn't known
             ("%F10000.log l -s m mlt -e --from 2018-02-18T12:37:00 -oc",
              "2018-02-18 12:37:09   ? >>> media-libs/mlt-6.4.1-r6\n\
               2018-02-27 15:10:05  43 >>> media-libs/mlt-6.4.1-r6\n\
               2018-02-27 16:48:40  39 >>> media-libs/mlt-6.4.1-r6\n"),
             // Check output of sync events
             ("%F10000.log l -ss --from 2018-03-07T10:42:00 --to 2018-03-07T14:00:00 -oc",
              "2018-03-07 11:37:05  38 Sync gentoo\n\
               2018-03-07 13:56:09  40 Sync gentoo\n"),
             ("%Fsync.log l -ss -oc",
              "2007-04-06 04:43:38    26:02 Sync gentoo-portage\n\
               2007-04-09 21:30:01    19:20 Sync gentoo-portage\n\
               2007-04-16 21:52:59    59:53 Sync gentoo-portage\n\
               2007-04-19 19:05:59    31:53 Sync gentoo-portage\n\
               2007-05-09 02:14:35  2:15:34 Sync gentoo-portage\n\
               2016-01-08 21:17:59       38 Sync gentoo\n\
               2016-01-10 23:31:57       49 Sync gentoo\n\
               2017-02-03 21:14:50       53 Sync gentoo\n\
               2017-02-06 21:06:18       55 Sync gentoo\n\
               2017-02-07 21:00:51     3:01 Sync gentoo\n\
               2017-02-09 22:06:12    13:22 Sync gentoo\n\
               2017-02-12 15:59:38       39 Sync gentoo\n\
               2017-02-12 20:43:23     1:48 Sync gentoo\n\
               2017-02-13 21:11:46     8:12 Sync gentoo\n\
               2017-03-11 20:59:57     8:26 Sync gentoo\n\
               2017-03-16 21:17:59       27 Sync gentoo\n\
               2017-03-22 22:13:50  1:00:31 Sync gentoo\n\
               2017-03-31 20:07:49     1:40 Sync gentoo\n\
               2017-04-03 20:10:55       24 Sync gentoo\n\
               2017-04-04 20:11:08       19 Sync gentoo\n\
               2020-06-16 08:53:33        6 Sync gentoo\n\
               2020-06-16 08:53:34        1 Sync moltonel\n\
               2020-06-16 15:40:57       14 Sync gentoo\n\
               2020-06-16 15:41:07       10 Sync moltonel\n\
               2020-06-16 15:54:16        6 Sync gentoo\n\
               2020-06-16 15:54:21        5 Sync steam-overlay\n\
               2020-06-16 15:54:24        3 Sync moltonel\n\
               2020-06-16 16:21:41        3 Sync gentoo\n\
               2020-06-16 16:21:42        1 Sync steam-overlay\n\
               2020-06-16 16:21:44        2 Sync moltonel\n\
               2020-06-16 20:58:00        7 Sync moltonel\n\
               2020-06-16 21:36:46        4 Sync gentoo\n\
               2020-06-16 21:36:47        1 Sync steam-overlay\n\
               2020-06-16 21:36:48        1 Sync moltonel\n\
               2020-06-17 20:24:00       30 Sync gentoo\n\
               2020-06-17 20:24:02        2 Sync steam-overlay\n\
               2020-06-17 20:24:03        1 Sync moltonel\n\
               2020-06-18 16:21:54        6 Sync gentoo\n\
               2020-06-18 16:21:55        1 Sync steam-overlay\n\
               2020-06-18 16:21:56        1 Sync moltonel\n"),
             // Check output of all events
             ("%F10000.log l --show a --from 2018-03-07T10:42:00 --to 2018-03-07T14:00:00 -oc",
              "2018-03-07 10:42:51       Emerge --backtrack=100 --quiet-build=y sys-apps/the_silver_searcher\n\
               2018-03-07 10:43:10    14 >>> sys-apps/the_silver_searcher-2.0.0\n\
               2018-03-07 11:36:27       Emerge --quiet-build=y --sync\n\
               2018-03-07 11:37:05    38 Sync gentoo\n\
               2018-03-07 11:38:29       Emerge --deep --backtrack=100 --quiet-build=y --ask --update --jobs=2 --newuse --verbose world\n\
               2018-03-07 12:49:09     2 <<< sys-apps/util-linux-2.30.2\n\
               2018-03-07 12:49:13  1:01 >>> sys-apps/util-linux-2.30.2-r1\n\
               2018-03-07 13:55:29       Emerge --quiet-build=y --sync\n\
               2018-03-07 13:56:09    40 Sync gentoo\n\
               2018-03-07 13:57:31       Emerge --update --jobs=2 --backtrack=100 --ask --quiet-build=y --verbose --deep --newuse world\n\
               2018-03-07 13:58:04       Emerge --update --verbose --newuse --quiet-build=y --deep --backtrack=100 world\n\
               2018-03-07 13:59:38     2 <<< dev-libs/nspr-4.17\n\
               2018-03-07 13:59:41    24 >>> dev-libs/nspr-4.18\n"),
             // skip first
             ("%F10000.log l client -oc --first 2",
              "2018-02-04 04:55:19  35:46 >>> mail-client/thunderbird-52.6.0\n\
               2018-02-04 05:42:48  47:29 >>> www-client/firefox-58.0.1\n\
               (skip last 10)             \n"),
             // skip first
             ("%F10000.log l client -oc --last 2",
              "(skip first 10)\n\
               2018-03-12 11:03:53  16 >>> kde-frameworks/kxmlrpcclient-5.44.0\n\
               2018-03-14 01:36:42  10 >>> www-client/falkon-24.08.3\n"),
             // Skip first and last
             ("%F10000.log l client -oc --first 4 --last 2",
              "(skip first 2)\n\
               2018-02-09 11:04:59  47:58 >>> mail-client/thunderbird-52.6.0-r1\n\
               2018-02-12 10:14:11     31 >>> kde-frameworks/kxmlrpcclient-5.43.0\n\
               (skip last 8)              \n"),
             // Skip silently
             ("%F10000.log l client -oc --first 4 --last 2 --showskip=n",
              "2018-02-09 11:04:59  47:58 >>> mail-client/thunderbird-52.6.0-r1\n\
               2018-02-12 10:14:11     31 >>> kde-frameworks/kxmlrpcclient-5.43.0\n")];
    for (a, o) in t {
        emlop(a).assert().stdout(o);
    }
}

#[test]
fn compressed() {
    // The important part here is that we're reading the gzip file
    let o = "831\t60:07:06\t4:20\t832\t38:31\t2\n";
    emlop("%Flog.gz s -st -ot").assert().stdout(o);
}

#[test]
fn starttime() {
    let o1 = emlop_out("%F10000.log l --dat=unix --dur=s");
    let o2 = emlop_out("%F10000.log l --dat=unix --dur=s --starttime");
    let mut lines = 0;
    for (l1, l2) in o1.lines().zip(o2.lines()) {
        lines += 1;
        let mut w1 = l1.split_ascii_whitespace();
        let mut w2 = l2.split_ascii_whitespace();
        let t1 = dbg!(w1.next()).expect("missing t1").parse::<u64>().expect("bad int t1");
        let d1 = dbg!(w1.next()).expect("missing d1");
        let t2 = dbg!(w2.next()).expect("missing t2").parse::<u64>().expect("bad int t2");
        let d2 = dbg!(w2.next()).expect("missing d2");
        assert!(d1 == d2);
        if d1 != "?" {
            assert!(t2 + d1.parse::<u64>().expect("bad int d1") == t1);
        }
    }
    assert!(lines > 500);
}

#[test]
fn timezone() {
    let t = [// UTC
             ("UTC",
              "2021-03-26 17:07:08 +00:00    20 >>> dev-libs/libksba-1.5.0\n\
               2021-03-26 17:08:20 +00:00  1:12 >>> sys-boot/grub-2.06_rc1\n\
               2021-03-29 10:57:14 +00:00    12 >>> sys-apps/install-xattr-0.8\n\
               2021-03-29 10:57:45 +00:00    31 >>> sys-devel/m4-1.4.18-r2\n"),
             // Moscow (east)
             ("Europe/Moscow",
              "2021-03-26 20:07:08 +03:00    20 >>> dev-libs/libksba-1.5.0\n\
               2021-03-26 20:08:20 +03:00  1:12 >>> sys-boot/grub-2.06_rc1\n\
               2021-03-29 13:57:14 +03:00    12 >>> sys-apps/install-xattr-0.8\n\
               2021-03-29 13:57:45 +03:00    31 >>> sys-devel/m4-1.4.18-r2\n"),
             // Marquesas island (west, non-whole)
             ("Pacific/Marquesas",
              "2021-03-26 07:37:08 -09:30    20 >>> dev-libs/libksba-1.5.0\n\
               2021-03-26 07:38:20 -09:30  1:12 >>> sys-boot/grub-2.06_rc1\n\
               2021-03-29 01:27:14 -09:30    12 >>> sys-apps/install-xattr-0.8\n\
               2021-03-29 01:27:45 -09:30    31 >>> sys-devel/m4-1.4.18-r2\n")];
    // Dublin (affected by DST)
    // FIXME: Hanling this properly will remain impossible until UtcOffset::local_offset_at
    // functionality is available after thread start (see
    // https://github.com/time-rs/time/issues/380). Until then, emlop's behavior is to display all
    // dates with the same offset (the curent one detected at program start) even though it should
    // be different at different dates. Not adding a unitest for this, as it would need to be
    // updated twice a year.
    //(&["-F", "test/emerge.dst.log", "l"],
    // "Europe/Dublin",
    // "2021-03-26 17:07:08 +00:00    20 >>> dev-libs/libksba-1.5.0\n\
    //  2021-03-26 17:08:20 +00:00  1:12 >>> sys-boot/grub-2.06_rc1\n\
    //  2021-03-29 11:57:14 +01:00    12 >>> sys-apps/install-xattr-0.8\n\
    //  2021-03-29 11:57:45 +01:00    31 >>> sys-devel/m4-1.4.18-r2\n"),
    for (t, o) in t {
        emlop("%Fdst.log l --date dto -oc").env("TZ", t).assert().stdout(o);
    }
}

/// Check Basic 'emlop p`. Not a hugely useful test, but it's something.
///
/// Ignored by default: depends on there being no currently running emerge.
#[ignore]
#[test]
fn predict_tty() {
    emlop("p %F10000.log").assert().code(1).stdout("No pretended merge found\n");
}

/// Ignored by default: depends on there being no currently running emerge.
#[ignore]
#[test]
fn predict_emerge_p() {
    let t =
        [// Check garbage input
         ("%F10000.log p --date unix -oc",
          "blah blah\n",
          format!("No pretended merge found\n"),
          1),
         // Check all-unknowns
         ("%F10000.log p --date unix -oc",
          "[ebuild   R   ~] dev-lang/unknown-1.42\n",
          format!("dev-lang/unknown-1.42              ? \n\
                   Estimate for 1 ebuild, 1 unknown  10 @ {}\n",
                  ts(10)),
          0),
         // Check that unknown ebuild don't wreck alignment. Remember that times are {:>9}
         ("%F10000.log p --date unix -oc",
          "[ebuild   R   ~] dev-qt/qtcore-5.9.4-r2\n\
               [ebuild   R   ~] dev-lang/unknown-1.42\n\
               [ebuild   R   ~] dev-qt/qtgui-5.9.4-r3\n",
          format!("dev-qt/qtcore-5.9.4-r2             3:45 \n\
                   dev-lang/unknown-1.42                 ? \n\
                   dev-qt/qtgui-5.9.4-r3              4:24 \n\
                   Estimate for 3 ebuilds, 1 unknown  8:19 @ {}\n",
                  ts(8 * 60 + 9 + 10)),
          0),
         // Check skip rows
         ("%F10000.log p --date unix -oc --show m --first 2",
          "[ebuild   R   ~] dev-qt/qtcore-1\n\
           [ebuild   R   ~] dev-qt/qtcore-2\n\
           [ebuild   R   ~] dev-qt/qtcore-3\n\
           [ebuild   R   ~] dev-qt/qtcore-4\n\
           [ebuild   R   ~] dev-qt/qtcore-5\n",
          "dev-qt/qtcore-1  3:45\n\
           dev-qt/qtcore-2  3:45\n\
           (skip last 3)        \n"
                                   .into(),
          0),
         ("%F10000.log p --date unix -oc --show m --first 2 --last 1",
          "[ebuild   R   ~] dev-qt/qtcore-1\n\
           [ebuild   R   ~] dev-qt/qtcore-2\n\
           [ebuild   R   ~] dev-qt/qtcore-3\n\
           [ebuild   R   ~] dev-qt/qtcore-4\n\
           [ebuild   R   ~] dev-qt/qtcore-5\n",
          "(skip first 1)\n\
           dev-qt/qtcore-2  3:45\n\
           (skip last 3)        \n"
                                   .into(),
          0)];
    for (a, i, o, e) in t {
        emlop(a).write_stdin(i).assert().code(e).stdout(o);
    }
}

#[test]
fn stats() {
    let t = [("%F10000.log s client -oc",
              "kde-frameworks/kxmlrpcclient  2        47       23  2   4  2\n\
               mail-client/thunderbird       2   1:23:44    41:52  2   6  3\n\
               www-client/chromium           3  21:41:24  7:42:07  3  12  3\n\
               www-client/falkon             1      6:02     6:02  0   0  ?\n\
               www-client/firefox            1     47:29    47:29  1   3  3\n\
               www-client/links              1        44       44  1   1  1\n\
               x11-apps/xlsclients           1        14       14  1   1  1\n",
              0),
             ("%Fsync.log s -ss -oc",
              "gentoo          22  1:43:13     10\n\
               gentoo-portage   5  4:32:42  31:53\n\
               moltonel         8       26      1\n\
               steam-overlay    5       10      1\n",
              0),
             ("%Fsync.log s -ss gentoo -oc",
              "gentoo          22  1:43:13     10\n\
               gentoo-portage   5  4:32:42  31:53\n",
              0),
             ("%F10000.log s client -sst -oc", "11  24:00:24  2:10:56  10  27  2\n", 0),
             ("%F10000.log s client -sa -oc",
              "450  267  20  163\n\
               \n\
               kde-frameworks/kxmlrpcclient  2        47       23  2   4  2\n\
               mail-client/thunderbird       2   1:23:44    41:52  2   6  3\n\
               www-client/chromium           3  21:41:24  7:42:07  3  12  3\n\
               www-client/falkon             1      6:02     6:02  0   0  ?\n\
               www-client/firefox            1     47:29    47:29  1   3  3\n\
               www-client/links              1        44       44  1   1  1\n\
               x11-apps/xlsclients           1        14       14  1   1  1\n\
               \n\
               11  24:00:24  2:10:56  10  27  2\n",
              0),
             ("%F10000.log s gentoo-sources --avg arith -oc",
              "sys-kernel/gentoo-sources  10  15:04  1:30  11  3:20  16\n",
              0),
             ("%F10000.log s gentoo-sources --avg median -oc",
              "sys-kernel/gentoo-sources  10  15:04  1:21  11  3:20  13\n",
              0),
             ("%F10000.log s gentoo-sources --avg weighted-arith -oc",
              "sys-kernel/gentoo-sources  10  15:04  1:31  11  3:20  17\n",
              0),
             ("%F10000.log s gentoo-sources --avg weighted-median -oc",
              "sys-kernel/gentoo-sources  10  15:04  1:22  11  3:20  15\n",
              0),
             ("%F10000.log s --from 2018-02-03T23:11:47 --to 2018-02-04 notfound -sa -oc", "", 1)];
    for (a, o, e) in t {
        emlop(a).assert().code(e).stdout(o);
    }
}

/// Test grouped stats. In addition to the usual check that the actual output matches the
/// expected one, we check that the expected outputs are consistent (y/m/w/d totals are the
/// same, and avg*count==tot).
#[test]
fn stats_grouped() {
    let t = [("%F10000.log s --duration s -sp gentoo-sources -oc -gy",
              "2018 sys-kernel/gentoo-sources  10  904  81  11  200  13\n"),
             ("%F10000.log s --duration s -sp gentoo-sources -oc -gm",
              "2018-02 sys-kernel/gentoo-sources  8  702   80  8  149  13\n\
               2018-03 sys-kernel/gentoo-sources  2  202  101  3   51  15\n"),
             ("%F10000.log s --duration s -sp gentoo-sources -oc -gw",
              "2018-05 sys-kernel/gentoo-sources  1   81   81  0   0   ?\n\
               2018-06 sys-kernel/gentoo-sources  2  192   96  3  66  14\n\
               2018-07 sys-kernel/gentoo-sources  2  198   99  0   0   ?\n\
               2018-08 sys-kernel/gentoo-sources  1   77   77  3  37  12\n\
               2018-09 sys-kernel/gentoo-sources  3  236   79  3  61  22\n\
               2018-10 sys-kernel/gentoo-sources  0    0    ?  1  23  23\n\
               2018-11 sys-kernel/gentoo-sources  1  120  120  1  13  13\n"),
             ("%F10000.log s --duration s -sp gentoo-sources -oc -gd",
              "2018-02-04 sys-kernel/gentoo-sources  1   81   81  0   0   ?\n\
               2018-02-05 sys-kernel/gentoo-sources  1   95   95  0   0   ?\n\
               2018-02-06 sys-kernel/gentoo-sources  0    0    ?  3  66  14\n\
               2018-02-08 sys-kernel/gentoo-sources  1   97   97  0   0   ?\n\
               2018-02-12 sys-kernel/gentoo-sources  1   80   80  0   0   ?\n\
               2018-02-18 sys-kernel/gentoo-sources  1  118  118  0   0   ?\n\
               2018-02-22 sys-kernel/gentoo-sources  0    0    ?  3  37  12\n\
               2018-02-23 sys-kernel/gentoo-sources  1   77   77  0   0   ?\n\
               2018-02-26 sys-kernel/gentoo-sources  1   79   79  0   0   ?\n\
               2018-02-27 sys-kernel/gentoo-sources  0    0    ?  2  46  23\n\
               2018-02-28 sys-kernel/gentoo-sources  1   75   75  0   0   ?\n\
               2018-03-01 sys-kernel/gentoo-sources  1   82   82  1  15  15\n\
               2018-03-05 sys-kernel/gentoo-sources  0    0    ?  1  23  23\n\
               2018-03-12 sys-kernel/gentoo-sources  1  120  120  1  13  13\n"),
             ("%F10000.log s --duration s -st -oc -gy", "2018 831  216426  260  832  2311  2\n"),
             ("%F10000.log s --duration s -st -oc -gm",
              "2018-02 533  158312  297  529  1497  2\n\
               2018-03 298   58114  195  303   814  2\n"),
             ("%F10000.log s --duration s -st -oc -gw",
              "2018-05  63  33577  532   60  132  2\n\
               2018-06  74  10070  136   68  225  3\n\
               2018-07 281  58604  208  258  709  2\n\
               2018-08  65  51276  788   69  197  2\n\
               2018-09  71  14737  207   95  316  3\n\
               2018-10 182  43782  240  187  519  2\n\
               2018-11  95   4380   46   95  213  2\n"),
             ("%F10000.log s --duration s -st -oc -gd",
              "2018-02-03  32   2741     85   32   70  2\n\
               2018-02-04  31  30836    994   28   62  2\n\
               2018-02-05   4    158     39    3    5  1\n\
               2018-02-06  44   4288     97   44  174  3\n\
               2018-02-07  15    857     57   13   28  2\n\
               2018-02-08   5    983    196    4    8  2\n\
               2018-02-09   6   3784    630    4   10  2\n\
               2018-02-12 208  29239    140  206  587  2\n\
               2018-02-13   1     19     19    0    0  ?\n\
               2018-02-14  44   4795    108   44   92  2\n\
               2018-02-15   3    137     45    3    6  2\n\
               2018-02-16  21  23914   1138    3   14  4\n\
               2018-02-18   4    500    125    2   10  5\n\
               2018-02-19   2  28977  14488    2    6  3\n\
               2018-02-20   2    488    244    1    2  2\n\
               2018-02-21  37   5522    149   36   93  2\n\
               2018-02-22  16  15396    962   23   82  3\n\
               2018-02-23   6    854    142    5   11  2\n\
               2018-02-24   2     39     19    2    3  1\n\
               2018-02-26  10   2730    273    9   18  2\n\
               2018-02-27  35   1403     40   49  175  3\n\
               2018-02-28   5    652    130   16   41  2\n\
               2018-03-01  13   9355    719   13   40  3\n\
               2018-03-02   5    510    102    5   37  7\n\
               2018-03-03   3     87     29    3    5  1\n\
               2018-03-05   9    168     18   21   84  4\n\
               2018-03-06   3  27746   9248    1    3  3\n\
               2018-03-07  46   2969     64   43   90  2\n\
               2018-03-08  74   5441     73   73  202  2\n\
               2018-03-09  50   7458    149   49  140  2\n\
               2018-03-12  95   4380     46   95  213  2\n"),
             ("%F10000.log s --duration s -ss -oc -gy", "2018 gentoo  150  4747  28\n"),
             ("%F10000.log s --duration s -ss -oc -gm",
              "2018-02 gentoo  90  2411  15\n\
               2018-03 gentoo  60  2336  28\n"),
             ("%F10000.log s --duration s -ss -oc -gw",
              "2018-05 gentoo   3   160  56\n\
               2018-06 gentoo  31   951  27\n\
               2018-07 gentoo  17   388  19\n\
               2018-08 gentoo  20   500  23\n\
               2018-09 gentoo  39  1899  49\n\
               2018-10 gentoo  36   728  21\n\
               2018-11 gentoo   4   121  32\n"),
             ("%F10000.log s --duration s -ss -oc -gd",
              "2018-02-03 gentoo   1   68   68\n\
               2018-02-04 gentoo   2   92   46\n\
               2018-02-05 gentoo   7  186   32\n\
               2018-02-06 gentoo   7  237   31\n\
               2018-02-07 gentoo   7  221   32\n\
               2018-02-08 gentoo   7  215   21\n\
               2018-02-09 gentoo   3   92   29\n\
               2018-02-12 gentoo   4   87   22\n\
               2018-02-13 gentoo   2   45   22\n\
               2018-02-14 gentoo   3   85   23\n\
               2018-02-15 gentoo   4   76   18\n\
               2018-02-16 gentoo   3   67   20\n\
               2018-02-18 gentoo   1   28   28\n\
               2018-02-19 gentoo   2   61   30\n\
               2018-02-20 gentoo   5  119   22\n\
               2018-02-21 gentoo   4   89   21\n\
               2018-02-22 gentoo   2   51   25\n\
               2018-02-23 gentoo   6  157   24\n\
               2018-02-24 gentoo   1   23   23\n\
               2018-02-26 gentoo   4   69   17\n\
               2018-02-27 gentoo   8  208   20\n\
               2018-02-28 gentoo   7  135   16\n\
               2018-03-01 gentoo   8  568   30\n\
               2018-03-02 gentoo  10  547   49\n\
               2018-03-03 gentoo   2  372  186\n\
               2018-03-05 gentoo   9   46    1\n\
               2018-03-06 gentoo   8  183   22\n\
               2018-03-07 gentoo   4  120   34\n\
               2018-03-08 gentoo   8  157   20\n\
               2018-03-09 gentoo   7  222   31\n\
               2018-03-12 gentoo   4  121   32\n")];
    let mut tots: HashMap<&str, (u64, u64, u64, u64)> = HashMap::new();
    let to_u64 = |v: &Vec<&str>, i: usize| v.get(i).unwrap().parse::<u64>().unwrap();
    for (a, o) in t {
        // Usual output matching
        emlop(a).assert().success().stdout(o);
        // Add up the "count" and "time" columns, grouped by timespan (year/month/week/day)
        for l in o.lines() {
            let cols: Vec<&str> = l.split_ascii_whitespace().collect();
            let tot = tots.entry(a.split_whitespace().last().unwrap()).or_insert((0, 0, 0, 0));
            match cols.len() {
                // Sync
                5 => {
                    (*tot).0 += to_u64(&cols, 2);
                    (*tot).1 += to_u64(&cols, 3);
                },
                // merge
                8 => {
                    (*tot).0 += to_u64(&cols, 2);
                    (*tot).1 += to_u64(&cols, 3);
                    (*tot).2 += to_u64(&cols, 5);
                    (*tot).3 += to_u64(&cols, 6);
                },
                // Total
                7 => {
                    (*tot).0 += to_u64(&cols, 1);
                    (*tot).1 += to_u64(&cols, 2);
                    (*tot).2 += to_u64(&cols, 4);
                    (*tot).3 += to_u64(&cols, 5);
                },
                _ => panic!("Unexpected col count {l}"),
            }
        }
    }
    // Because we run the same test for each timespan, overall totals should match
    assert!(tots.iter().all(|(_, c)| c == tots.get("-gy").unwrap()),
            "Timespans should match {:?}",
            tots);
}

/// Test behaviour when clock goes backward between merge start and merge end. Likely to happen
/// when you're bootstrapping an Gentoo and setting the time halfway through.
#[test]
fn negative_merge_time() {
    let t = [// For `log` we show an unknown time.
             ("%Fnegtime.log l -sms -oc",
              format!("2019-06-05 08:32:10  1:06 Sync gentoo\n\
                       2019-06-05 11:26:54  5:56 >>> kde-plasma/kwin-5.15.5\n\
                       2019-06-06 02:11:48    26 >>> kde-apps/libktnef-19.04.1\n\
                       2019-06-06 02:16:01    34 >>> net-misc/chrony-3.3\n\
                       2019-06-05 10:18:28     ? Sync gentoo\n\
                       2019-06-05 10:21:02     ? >>> kde-plasma/kwin-5.15.5\n\
                       2019-06-08 21:33:36  3:10 >>> kde-plasma/kwin-5.15.5\n")),
             // For `stats` the negative merge time is used for count but ignored for tottime/predtime.
             ("%Fnegtime.log s -sstp -oc",
              format!("gentoo  2  1:06  1:06\n\
                       \n\
                       kde-apps/libktnef  1    26    26  0  0  ?\n\
                       kde-plasma/kwin    3  9:06  4:33  2  3  1\n\
                       net-misc/chrony    1    34    34  0  0  ?\n\
                       \n\
                       5  10:06  2:01  2  3  1\n"))];
    for (a, o) in t {
        emlop(a).assert().success().stdout(o);
    }
}

/// Same as negative_merge_time() but for predict command.
/// For `pred` the negative merge time is ignored.
#[test]
fn negative_merge_time_pred() {
    let a = "%Fnegtime.log p -stm --date unix -oc";
    let i = "[ebuild   R   ~] kde-plasma/kwin-5.15.5\n";
    let o = format!("kde-plasma/kwin-5.15.5  4:33 \n\
                     Estimate for 1 ebuild   4:33 @ {}\n",
                    ts(4 * 60 + 33));
    emlop(a).write_stdin(i).assert().success().stdout(o);
}

#[test]
fn exit_status() {
    // 0: no problem
    // 1: command ran properly but didn't find anything
    // 2: user or program error
    let t = [// Help, version, badarg (clap)
             ("-h", 0),
             ("-V", 0),
             ("l -h", 0),
             ("", 2),
             ("s --foo", 2),
             ("badcmd", 2),
             ("--utc", 2),
             // Bad arguments (emlop)
             ("l --logfile notfound", 2),
             ("s --logfile notfound", 2),
             ("p --logfile notfound", 2),
             ("l bad_regex_[a-z", 2),
             ("s bad_regex_[a-z", 2),
             ("p bad_regex_[a-z", 2),
             // Normal behaviour
             ("%F10000.log p", 1),
             ("%F10000.log l", 0),
             ("%F10000.log l -sm", 0),
             ("%F10000.log l -e icu", 0),
             ("%F10000.log l -e unknown", 1),
             ("%F10000.log l --from 2018-09-28", 1),
             ("%F10000.log l -sm --from 2018-09-28", 1),
             ("%F10000.log s", 0),
             ("%F10000.log s -e icu", 0),
             ("%F10000.log s -e unknown", 1)];
    for (a, e) in t {
        emlop(a).assert().code(e);
    }
}
