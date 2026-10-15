import ghidra.app.script.GhidraScript;
import ghidra.program.model.listing.*;
import ghidra.program.model.address.*;
import java.util.*;
public class RobotsInputFieldWritersEvidence extends GhidraScript {
 public void run() throws Exception {
  Listing l=currentProgram.getListing();
  Address start=toAddr(0x00400000L), end=toAddr(0x00590000L);
  InstructionIterator it=l.getInstructions(start,true);
  while(it.hasNext()){
   Instruction ins=it.next(); if(ins.getAddress().compareTo(end)>0)break;
   String s=ins.toString().toLowerCase();
   if(!s.contains("0x500")) continue;
   Function f=getFunctionContaining(ins.getAddress());
   println(ins.getAddress()+" "+ins+" FUNC="+(f==null?"<none>":f.getName()+"@"+f.getEntryPoint()));
  }
 }
}