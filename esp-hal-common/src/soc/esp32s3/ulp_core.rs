//! Control the ULP RISCV core

use esp32s3 as pac;

use crate::peripheral::{Peripheral, PeripheralRef};

extern "C" {
    fn ets_delay_us(delay: u32);
}

#[derive(Debug, Clone, Copy)]
pub enum UlpCoreWakeupSource {
    HpCpu,
}

pub struct UlpCore<'d> {
    _lp_core: PeripheralRef<'d, crate::soc::peripherals::ULP_RISCV_CORE>,
}

impl<'d> UlpCore<'d> {
    pub fn new(lp_core: impl Peripheral<P = crate::soc::peripherals::ULP_RISCV_CORE> + 'd) -> Self {
        crate::into_ref!(lp_core);
        Self { _lp_core: lp_core }
    }

    pub fn stop(&mut self) {
        ulp_stop();
    }

    pub fn run(&mut self, wakeup_src: UlpCoreWakeupSource) {
        ulp_run(wakeup_src);
    }
}

fn ulp_stop() {
    let rtc_cntl = unsafe { &*pac::RTC_CNTL::PTR };
    rtc_cntl
        .ulp_cp_timer
        .modify(|_, w| w.ulp_cp_slp_timer_en().clear_bit());

    // suspends the ulp operation
    rtc_cntl.cocpu_ctrl.modify(|_, w| w.cocpu_done().set_bit());

    // Resets the processor
    rtc_cntl
        .cocpu_ctrl
        .modify(|_, w| w.cocpu_shut_reset_en().set_bit());

    unsafe {
        ets_delay_us(20);
    }

    // above doesn't seem to halt the ULP core - this will
    rtc_cntl
        .cocpu_ctrl
        .modify(|_, w| w.cocpu_clkgate_en().clear_bit());
}

fn ulp_run(wakeup_src: UlpCoreWakeupSource) {
    let rtc_cntl = unsafe { &*pac::RTC_CNTL::PTR };

    // Reset COCPU when power on
    rtc_cntl
        .cocpu_ctrl
        .modify(|_, w| w.cocpu_shut_reset_en().set_bit());

    // The coprocessor cpu trap signal doesnt have a stable reset value,
    // force ULP-RISC-V clock on to stop RTC_COCPU_TRAP_TRIG_EN from waking the CPU
    rtc_cntl
        .cocpu_ctrl
        .modify(|_, w| w.cocpu_clk_fo().set_bit());

    // Disable ULP timer
    rtc_cntl
        .ulp_cp_timer
        .modify(|_, w| w.ulp_cp_slp_timer_en().clear_bit());

    // wait for at least 1 RTC_SLOW_CLK cycle
    unsafe {
        ets_delay_us(20);
    }

    // We do not select RISC-V as the Coprocessor here as this could lead to a hang
    // in the main CPU. Instead, we reset RTC_CNTL_COCPU_SEL after we have enabled
    // the ULP timer.
    //
    // IDF-4510

    // Select ULP-RISC-V to send the DONE signal
    rtc_cntl
        .cocpu_ctrl
        .modify(|_, w| w.cocpu_done_force().set_bit());

    // Set the CLKGATE_EN signal
    rtc_cntl
        .cocpu_ctrl
        .modify(|_, w| w.cocpu_clkgate_en().set_bit());

    ulp_config_wakeup_source(wakeup_src);

    // Select RISC-V as the ULP_TIMER trigger target
    // Selecting the RISC-V as the Coprocessor at the end is a workaround
    // for the hang issue recorded in IDF-4510.
    rtc_cntl.cocpu_ctrl.modify(|_, w| w.cocpu_sel().clear_bit());

    // Clear any spurious wakeup trigger interrupts upon ULP startup
    unsafe {
        ets_delay_us(20);
    }

    rtc_cntl.int_clr_rtc.write(|w| {
        w.cocpu_int_clr()
            .set_bit()
            .cocpu_trap_int_clr()
            .set_bit()
            .ulp_cp_int_clr()
            .set_bit()
    });

    rtc_cntl
        .cocpu_ctrl
        .modify(|_, w| w.cocpu_clkgate_en().set_bit());
}

fn ulp_config_wakeup_source(wakeup_src: UlpCoreWakeupSource) {
    match wakeup_src {
        UlpCoreWakeupSource::HpCpu => {
            // use timer to wake up
            let rtc_cntl = unsafe { &*pac::RTC_CNTL::PTR };
            rtc_cntl
                .ulp_cp_ctrl
                .modify(|_, w| w.ulp_cp_force_start_top().clear_bit());
            rtc_cntl
                .ulp_cp_timer
                .modify(|_, w| w.ulp_cp_slp_timer_en().set_bit());
        }
    }
}
