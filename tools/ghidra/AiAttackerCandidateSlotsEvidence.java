import ghidra.app.decompiler.*;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.listing.Function;
import java.util.*;
public class AiAttackerCandidateSlotsEvidence extends GhidraScript {
 record Row(String n,long v){}
 long p(long a)throws Exception{return Integer.toUnsignedLong(getInt(toAddr(a)));}
 @Override public void run()throws Exception{
  Row[] rows={
   new Row("MonsterBase",0x005E2920L),new Row("Monster2Rockets",0x005E2D58L),
   new Row("ConstructionBot",0x005E3A00L),new Row("Eb07MineBot",0x005E3FA0L),
   new Row("Eb11MagnaBot",0x005E53B8L),new Row("Eb12EvilBot",0x005E5F00L),
   new Row("Eb13KnightBot",0x005E6338L),new Row("Eb14Minion",0x005E61D0L),
   new Row("Eb15Launcher",0x005E64A0L),new Row("Eb16KnuckleBot",0x005E6608L),
   new Row("Ef03EvilBot",0x005E68D8L),new Row("Eq03Spider",0x005E5D98L),
   new Row("Ew08Flambe",0x005E5690L),new Row("Ew08FlambeLarge",0x005E57F8L),
   new Row("Ew09Armoured",0x005E50E0L),new Row("Ew10Minion",0x005E6068L),
   new Row("Ew11FatBot",0x005E6770L),new Row("GuardBot",0x005E3B68L),
   new Row("JailBotLarge",0x005E3898L),new Row("JailBotNormal",0x005E3730L),
   new Row("SawBot",0x005E3028L),new Row("SecurityBot",0x005E3E38L),
   new Row("ShieldBot",0x005E3CD0L),new Row("ShuntBot",0x005E3190L),
   new Row("ShuntBotBoss",0x005E32F8L),new Row("SpikeBot",0x005E2EC0L),
   new Row("ThiefBot",0x005E4108L),new Row("MalfBot",0x005E4E08L)
  };
  Set<Long> fs=new LinkedHashSet<>();
  for(Row r:rows){long a=p(r.v()+0x14c),b=p(r.v()+0x15c);println(String.format("%s +14C=0x%08X +15C=0x%08X",r.n(),a,b));fs.add(a);fs.add(b);}
  DecompInterface d=new DecompInterface();d.openProgram(currentProgram);
  for(long x:fs){Function f=getFunctionContaining(toAddr(x));println(String.format("\n=== 0x%08X %s ===",x,f==null?"<missing>":f.getName()));if(f!=null){DecompileResults z=d.decompileFunction(f,90,monitor);if(z.decompileCompleted()&&z.getDecompiledFunction()!=null)println(z.getDecompiledFunction().getC());}}
  d.dispose();
 }
}