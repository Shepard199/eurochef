import ghidra.app.script.GhidraScript;

public class AiHandlerBehaviorMatrix extends GhidraScript {
    private static class Row {
        final String name; final long vt;
        Row(String name, long vt) { this.name=name; this.vt=vt; }
    }
    private long ptr(long a) throws Exception { return Integer.toUnsignedLong(getInt(toAddr(a))); }
    @Override public void run() throws Exception {
        Row[] rows = {
            new Row("Monster",0x005E2920L), new Row("2Rockets",0x005E2D58L),
            new Row("ConstructionBot",0x005E3A00L), new Row("DogBot",0x005E2BF0L),
            new Row("EB07_MineBot",0x005E3FA0L), new Row("EB10_RollerBot",0x005E5520L),
            new Row("EB11_MagnaBot",0x005E53B8L), new Row("EB12_EvilBot",0x005E5F00L),
            new Row("EB13_KnightBot",0x005E6338L), new Row("EB14_Minion",0x005E61D0L),
            new Row("EB15_Launcher",0x005E64A0L), new Row("EB16_KnuckleBot",0x005E6608L),
            new Row("EF01_Mine",0x005E5AC8L), new Row("EF03_EvilBot",0x005E68D8L),
            new Row("EM07_PiranhaBot",0x005E6A40L), new Row("EP02_Turret",0x005E4828L),
            new Row("EP04_Turret",0x005E49A0L), new Row("EP05_Turret",0x005E4B18L),
            new Row("EP06_Turret",0x005E4C90L), new Row("EQ02_MineBot",0x005E43E0L),
            new Row("EQ03_Spider",0x005E5D98L), new Row("EQ04_Mine",0x005E5960L),
            new Row("EW07_Dodgem",0x005E4F70L), new Row("EW08_Flambe",0x005E5690L),
            new Row("EW08_FlambeLarge",0x005E57F8L), new Row("EW09_Armoured",0x005E50E0L),
            new Row("EW10_Minion",0x005E6068L), new Row("EW11_FatBot",0x005E6770L),
            new Row("GuardBot",0x005E3B68L), new Row("JailBotLarge",0x005E3898L),
            new Row("JailBotNormal",0x005E3730L), new Row("MalfBot",0x005E4E08L),
            new Row("SawBot",0x005E3028L), new Row("SecurityBot",0x005E3E38L),
            new Row("ShieldBot",0x005E3CD0L), new Row("ShuntBot",0x005E3190L),
            new Row("ShuntBotBoss",0x005E32F8L), new Row("SpikeBot",0x005E2EC0L),
            new Row("SpinTop",0x005E4270L), new Row("Sweeper",0x005E4548L),
            new Row("TestAnimBot",0x005E6BA8L), new Row("ThiefBot",0x005E4108L),
            new Row("TurretBot",0x005E3460L), new Row("Npc",0x005E7048L),
            new Row("NpcFender",0x005E71B8L)
        };
        int[] slots={0x08,0x34,0x50,0xC8,0x100,0x104,0x108,0x10C,0x110,0x114,0x118,0x11C,0x120,0x124,0x128,0x12C,0x130,0x138,0x13C,0x140};
        print("class\tvtable"); for(int s:slots) print(String.format("\t+%03X",s)); println("");
        for(Row r:rows){
            print(r.name+String.format("\t%08X",r.vt));
            for(int s:slots) print(String.format("\t%08X",ptr(r.vt+s)));
            println("");
        }
    }
}
